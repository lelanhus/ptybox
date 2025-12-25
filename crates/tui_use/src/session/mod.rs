use crate::model::PROTOCOL_VERSION;
use crate::model::{Action, ActionType, Observation, RunId, SessionId, TerminalSize};
use crate::policy::apply_env_policy;
use crate::runner::RunnerError;
use crate::terminal::Terminal;
#[cfg(unix)]
use nix::fcntl::{fcntl, FcntlArg, OFlag};
#[cfg(unix)]
use nix::sys::signal::{killpg, Signal};
#[cfg(unix)]
use nix::unistd::Pid;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Session {
    run_id: RunId,
    session_id: SessionId,
    terminal: Arc<Mutex<Terminal>>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    reader: Box<dyn Read + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    started_at: Instant,
}

#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub size: TerminalSize,
    pub run_id: RunId,
    pub env: crate::model::policy::EnvPolicy,
}

impl Session {
    pub fn spawn(config: SessionConfig) -> Result<Self, RunnerError> {
        let system = native_pty_system();
        let pty_size = PtySize {
            rows: config.size.rows,
            cols: config.size.cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = system
            .openpty(pty_size)
            .map_err(|err| RunnerError::io("E_IO", "failed to open pty", err))?;

        let mut cmd = CommandBuilder::new(&config.command);
        cmd.args(&config.args);
        if let Some(cwd) = &config.cwd {
            cmd.cwd(cwd);
        }
        apply_env_policy(&config.env, &mut cmd)?;

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|err| RunnerError::io("E_IO", "failed to spawn command", err))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| RunnerError::io("E_IO", "failed to clone pty reader", err))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|err| RunnerError::io("E_IO", "failed to take pty writer", err))?;

        #[cfg(unix)]
        {
            if let Some(fd) = pair.master.as_raw_fd() {
                let flags = OFlag::from_bits_truncate(
                    fcntl(fd, FcntlArg::F_GETFL)
                        .map_err(|err| RunnerError::io("E_IO", "failed to get fd flags", err))?,
                );
                let new_flags = flags | OFlag::O_NONBLOCK;
                fcntl(fd, FcntlArg::F_SETFL(new_flags))
                    .map_err(|err| RunnerError::io("E_IO", "failed to set nonblocking", err))?;
            }
        }

        let terminal = Terminal::new(config.size);

        Ok(Self {
            run_id: config.run_id,
            session_id: SessionId::new(),
            terminal: Arc::new(Mutex::new(terminal)),
            master: pair.master,
            writer,
            reader,
            child,
            started_at: Instant::now(),
        })
    }

    pub fn send(&mut self, action: &Action) -> Result<(), RunnerError> {
        match action.action_type {
            ActionType::Key => {
                let key = action
                    .payload
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        RunnerError::protocol_with_context(
                            "E_PROTOCOL",
                            "missing or invalid 'key' field in key action payload",
                            serde_json::json!({
                                "received_payload": action.payload,
                                "expected": {"key": "string"},
                                "supported_keys": ["Enter", "Up", "Down", "Left", "Right", "Tab", "Escape", "Backspace", "Delete", "Home", "End", "PageUp", "PageDown", "or single character"],
                                "example": {"type": "key", "payload": {"key": "Enter"}}
                            }),
                        )
                    })?;
                let bytes = key_to_bytes(key)?;
                self.writer
                    .write_all(&bytes)
                    .map_err(|err| RunnerError::io("E_IO", "failed to write key", err))?;
                self.writer
                    .flush()
                    .map_err(|err| RunnerError::io("E_IO", "failed to flush key", err))?;
                Ok(())
            }
            ActionType::Text => {
                let text = action
                    .payload
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        RunnerError::protocol_with_context(
                            "E_PROTOCOL",
                            "missing or invalid 'text' field in text action payload",
                            serde_json::json!({
                                "received_payload": action.payload,
                                "expected": {"text": "string"},
                                "example": {"type": "text", "payload": {"text": "hello world"}}
                            }),
                        )
                    })?;
                self.writer
                    .write_all(text.as_bytes())
                    .map_err(|err| RunnerError::io("E_IO", "failed to write text", err))?;
                self.writer
                    .flush()
                    .map_err(|err| RunnerError::io("E_IO", "failed to flush text", err))?;
                Ok(())
            }
            ActionType::Resize => {
                // Terminal dimensions are always small, safe to truncate
                #[allow(clippy::cast_possible_truncation)]
                let rows = action
                    .payload
                    .get("rows")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        RunnerError::protocol_with_context(
                            "E_PROTOCOL",
                            "missing or invalid 'rows' field in resize action payload",
                            serde_json::json!({
                                "received_payload": action.payload,
                                "expected": {"rows": "number (u16)", "cols": "number (u16)"},
                                "example": {"type": "resize", "payload": {"rows": 24, "cols": 80}}
                            }),
                        )
                    })? as u16;
                #[allow(clippy::cast_possible_truncation)]
                let cols = action
                    .payload
                    .get("cols")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        RunnerError::protocol_with_context(
                            "E_PROTOCOL",
                            "missing or invalid 'cols' field in resize action payload",
                            serde_json::json!({
                                "received_payload": action.payload,
                                "expected": {"rows": "number (u16)", "cols": "number (u16)"},
                                "example": {"type": "resize", "payload": {"rows": 24, "cols": 80}}
                            }),
                        )
                    })? as u16;
                self.master
                    .resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    })
                    .map_err(|err| RunnerError::io("E_IO", "failed to resize pty", err))?;
                let mut terminal = self
                    .terminal
                    .lock()
                    .map_err(|_| RunnerError::internal("E_INTERNAL", "terminal lock poisoned"))?;
                terminal.resize(TerminalSize { rows, cols });
                Ok(())
            }
            ActionType::Wait => Ok(()),
            ActionType::Terminate => self.terminate(),
        }
    }

    pub fn observe(&mut self, timeout: Duration) -> Result<Observation, RunnerError> {
        let mut total = Vec::new();
        let deadline = Instant::now() + timeout;
        loop {
            let mut read_buffer = vec![0u8; 4096];
            match self.reader.read(&mut read_buffer) {
                Ok(0) => break,
                Ok(count) => {
                    read_buffer.truncate(count);
                    total.extend_from_slice(&read_buffer);
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(err) => return Err(RunnerError::io("E_IO", "failed to read pty", err)),
            }
            if Instant::now() >= deadline {
                break;
            }
        }

        let transcript_delta = if total.is_empty() {
            None
        } else {
            match std::str::from_utf8(&total) {
                Ok(value) => Some(value.to_string()),
                Err(err) => {
                    return Err(RunnerError::terminal_parse(
                        "E_TERMINAL_PARSE",
                        "terminal output was not valid UTF-8",
                        err,
                        Some(err.valid_up_to()),
                    ));
                }
            }
        };

        let mut terminal = self
            .terminal
            .lock()
            .map_err(|_| RunnerError::internal("E_INTERNAL", "terminal lock poisoned"))?;
        terminal.process_bytes(&total);
        let snapshot = terminal.snapshot()?;
        Ok(Observation {
            protocol_version: PROTOCOL_VERSION,
            run_id: self.run_id,
            session_id: self.session_id,
            // Elapsed time is always well under u64::MAX
            #[allow(clippy::cast_possible_truncation)]
            timestamp_ms: { self.started_at.elapsed().as_millis() as u64 },
            screen: snapshot,
            transcript_delta,
            events: Vec::new(),
        })
    }

    pub fn wait_for_exit(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<portable_pty::ExitStatus>, RunnerError> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(Some(status)),
                Ok(None) => {
                    if Instant::now() > deadline {
                        return Ok(None);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => {
                    return Err(RunnerError::io("E_IO", "failed to wait for child", err));
                }
            }
        }
    }

    pub fn terminate(&mut self) -> Result<(), RunnerError> {
        #[cfg(unix)]
        if let Some(pid) = self.child.process_id() {
            // Process IDs are always positive and fit in i32
            #[allow(clippy::cast_possible_wrap)]
            let pgid = Pid::from_raw(pid as i32);
            return signal_process_group(pgid, Signal::SIGTERM);
        }

        self.child
            .kill()
            .map_err(|err| RunnerError::io("E_IO", "failed to terminate child", err))
    }

    pub fn terminate_process_group(
        &mut self,
        grace: Duration,
    ) -> Result<Option<portable_pty::ExitStatus>, RunnerError> {
        #[cfg(unix)]
        if let Some(pid) = self.child.process_id() {
            // Process IDs are always positive and fit in i32
            #[allow(clippy::cast_possible_wrap)]
            let pgid = Pid::from_raw(pid as i32);
            signal_process_group(pgid, Signal::SIGTERM)?;
            if let Some(status) = self.wait_for_exit(grace)? {
                return Ok(Some(status));
            }
            signal_process_group(pgid, Signal::SIGKILL)?;
            return self.wait_for_exit(Duration::from_millis(200));
        }

        self.terminate()?;
        self.wait_for_exit(grace)
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
}

#[cfg(unix)]
fn signal_process_group(pgid: Pid, signal: Signal) -> Result<(), RunnerError> {
    match killpg(pgid, signal) {
        // ESRCH means process already gone, which is fine
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(err) => Err(RunnerError::io(
            "E_IO",
            "failed to signal process group",
            err,
        )),
    }
}

const SUPPORTED_KEYS: &[&str] = &[
    "Enter",
    "Up",
    "Down",
    "Left",
    "Right",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
];

fn key_to_bytes(key: &str) -> Result<Vec<u8>, RunnerError> {
    let bytes = match key {
        "Enter" => vec![b'\r'],
        "Up" => b"\x1b[A".to_vec(),
        "Down" => b"\x1b[B".to_vec(),
        "Right" => b"\x1b[C".to_vec(),
        "Left" => b"\x1b[D".to_vec(),
        "Tab" => vec![b'\t'],
        "Escape" => vec![0x1b],
        "Backspace" => vec![0x7f],
        "Delete" => b"\x1b[3~".to_vec(),
        "Home" => b"\x1b[H".to_vec(),
        "End" => b"\x1b[F".to_vec(),
        "PageUp" => b"\x1b[5~".to_vec(),
        "PageDown" => b"\x1b[6~".to_vec(),
        _ => {
            if key.len() == 1 {
                return Ok(key.as_bytes().to_vec());
            }
            return Err(RunnerError::protocol_with_context(
                "E_PROTOCOL",
                format!("unsupported key '{key}'"),
                serde_json::json!({
                    "received_key": key,
                    "supported_keys": SUPPORTED_KEYS,
                    "note": "Single characters are also supported (e.g., 'a', '1', '@')",
                    "example": {"type": "key", "payload": {"key": "Enter"}}
                }),
            ));
        }
    };
    Ok(bytes)
}

impl Session {
    /// Best-effort cleanup of the child process. Used by Drop.
    /// All errors are silently ignored since Drop cannot propagate errors.
    fn cleanup_process_best_effort(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.child.process_id() {
            // Process IDs are always positive and fit in i32
            #[allow(clippy::cast_possible_wrap)]
            let pgid = Pid::from_raw(pid as i32);

            // Try graceful termination first
            let _ = signal_process_group(pgid, Signal::SIGTERM);

            // Wait briefly for graceful exit (100ms max to keep Drop fast)
            let deadline = Instant::now() + Duration::from_millis(100);
            while Instant::now() < deadline {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }

            // Still alive, force kill (don't wait for SIGKILL to complete)
            let _ = signal_process_group(pgid, Signal::SIGKILL);
        }

        #[cfg(not(unix))]
        {
            let _ = self.child.kill();
        }
    }
}

impl Drop for Session {
    /// Performs best-effort cleanup of the child process.
    ///
    /// Sends SIGTERM to the process group, waits up to 100ms for graceful exit,
    /// then sends SIGKILL if still alive. All errors are silently ignored.
    ///
    /// For controlled termination with error handling, use `terminate_process_group()`
    /// before the Session is dropped.
    fn drop(&mut self) {
        self.cleanup_process_best_effort();
    }
}
