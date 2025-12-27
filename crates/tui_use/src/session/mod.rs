//! PTY session management for driving TUI applications.
//!
//! This module provides [`Session`] for spawning and interacting with
//! terminal applications via a pseudo-terminal (PTY). It handles the
//! low-level details of PTY creation, non-blocking I/O, terminal emulation,
//! and process lifecycle management.
//!
//! # Key Types
//!
//! - [`Session`] - The main PTY session handle for driving a TUI application
//! - [`SessionConfig`] - Configuration for spawning a new session
//!
//! # Key Operations
//!
//! - [`Session::spawn`] - Create a new PTY session with the given configuration
//! - [`Session::send`] - Send actions (keys, text, resize, terminate) to the session
//! - [`Session::observe`] - Read terminal output and capture a screen snapshot
//! - [`Session::terminate`] - Send SIGTERM to gracefully stop the process
//! - [`Session::terminate_process_group`] - Graceful termination with SIGKILL fallback
//! - [`Session::close`] - Explicit cleanup with full error handling
//!
//! # Example
//!
//! ```no_run
//! use tui_use::session::{Session, SessionConfig};
//! use tui_use::model::{Action, ActionType, RunId, TerminalSize};
//! use std::time::Duration;
//!
//! # fn example() -> Result<(), tui_use::runner::RunnerError> {
//! // Spawn a new session
//! let config = SessionConfig {
//!     command: "/bin/cat".to_string(),
//!     args: vec![],
//!     cwd: None,
//!     size: TerminalSize::default(),
//!     run_id: RunId::new(),
//!     env: Default::default(),
//! };
//! let mut session = Session::spawn(config)?;
//!
//! // Send some text input
//! let action = Action {
//!     action_type: ActionType::Text,
//!     payload: serde_json::json!({"text": "hello"}),
//! };
//! session.send(&action)?;
//!
//! // Observe the terminal output
//! let observation = session.observe(Duration::from_millis(100))?;
//! println!("Screen: {:?}", observation.screen.lines);
//!
//! // Clean shutdown
//! session.close(Duration::from_millis(500))?;
//! # Ok(())
//! # }
//! ```
//!
//! # Process Management
//!
//! Sessions automatically clean up child processes when dropped, but for
//! proper error handling use [`Session::close`] or [`Session::terminate_process_group`]
//! before the session goes out of scope.

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

/// Minimum terminal rows for resize validation.
const MIN_TERMINAL_ROWS: u16 = 1;
/// Maximum terminal rows for resize validation.
const MAX_TERMINAL_ROWS: u16 = 500;
/// Minimum terminal columns for resize validation.
const MIN_TERMINAL_COLS: u16 = 1;
/// Maximum terminal columns for resize validation.
const MAX_TERMINAL_COLS: u16 = 500;

// =============================================================================
// Payload Extraction Helpers
// =============================================================================

/// Extension trait for extracting typed values from JSON payloads with consistent error handling.
trait PayloadExt {
    /// Extract a string field from the payload.
    fn extract_str(&self, key: &str, context: &str) -> Result<&str, RunnerError>;
    /// Extract a u64 field from the payload.
    fn extract_u64(&self, key: &str, context: &str) -> Result<u64, RunnerError>;
}

impl PayloadExt for serde_json::Value {
    fn extract_str(&self, key: &str, context: &str) -> Result<&str, RunnerError> {
        self.get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                RunnerError::protocol(
                    "E_PROTOCOL",
                    format!("missing or invalid '{key}' field in {context} payload"),
                    serde_json::json!({
                        "received_payload": self,
                        "expected": {key: "string"},
                    }),
                )
            })
    }

    fn extract_u64(&self, key: &str, context: &str) -> Result<u64, RunnerError> {
        self.get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                RunnerError::protocol(
                    "E_PROTOCOL",
                    format!("missing or invalid '{key}' field in {context} payload"),
                    serde_json::json!({
                        "received_payload": self,
                        "expected": {key: "number"},
                    }),
                )
            })
    }
}

/// A PTY-backed session for driving a TUI application.
///
/// # Example
/// ```no_run
/// use tui_use::session::{Session, SessionConfig};
/// use tui_use::model::{RunId, TerminalSize};
/// use tui_use::runner::RunnerError;
/// use std::time::Duration;
///
/// fn example() -> Result<(), RunnerError> {
///     let config = SessionConfig {
///         command: "/bin/cat".to_string(),
///         args: vec![],
///         cwd: None,
///         size: TerminalSize::default(),
///         run_id: RunId::new(),
///         env: Default::default(),
///     };
///     let mut session = Session::spawn(config)?;
///     let observation = session.observe(Duration::from_millis(50))?;
///     session.terminate()?;
///     Ok(())
/// }
/// ```
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

/// Configuration for spawning a session.
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// Command to execute (absolute path recommended).
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Working directory.
    pub cwd: Option<String>,
    /// Initial terminal size.
    pub size: TerminalSize,
    /// Run identifier for this session.
    pub run_id: RunId,
    /// Environment variable policy.
    pub env: crate::model::policy::EnvPolicy,
}

impl Session {
    /// Spawn a new PTY session with the given configuration.
    ///
    /// # Errors
    /// Returns `RunnerError` with code `E_IO` if PTY creation or command spawn fails.
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

    /// Send an action to the terminal session.
    ///
    /// Handles key presses, text input, resize, wait (no-op), and terminate.
    ///
    /// # Errors
    /// - `E_IO`: Failed to write to PTY
    /// - `E_PROTOCOL`: Invalid action payload
    pub fn send(&mut self, action: &Action) -> Result<(), RunnerError> {
        match action.action_type {
            ActionType::Key => {
                let key = action.payload.extract_str("key", "key action")?;
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
                let text = action.payload.extract_str("text", "text action")?;
                self.writer
                    .write_all(text.as_bytes())
                    .map_err(|err| RunnerError::io("E_IO", "failed to write text", err))?;
                self.writer
                    .flush()
                    .map_err(|err| RunnerError::io("E_IO", "failed to flush text", err))?;
                Ok(())
            }
            ActionType::Resize => {
                let rows_u64 = action.payload.extract_u64("rows", "resize action")?;
                let cols_u64 = action.payload.extract_u64("cols", "resize action")?;

                // Validate bounds before conversion to prevent silent truncation
                let rows = u16::try_from(rows_u64).map_err(|_| {
                    RunnerError::protocol(
                        "E_PROTOCOL",
                        format!(
                            "rows value {} exceeds maximum u16 value {}",
                            rows_u64,
                            u16::MAX
                        ),
                        serde_json::json!({
                            "received": rows_u64,
                            "max": u16::MAX
                        }),
                    )
                })?;
                let cols = u16::try_from(cols_u64).map_err(|_| {
                    RunnerError::protocol(
                        "E_PROTOCOL",
                        format!(
                            "cols value {} exceeds maximum u16 value {}",
                            cols_u64,
                            u16::MAX
                        ),
                        serde_json::json!({
                            "received": cols_u64,
                            "max": u16::MAX
                        }),
                    )
                })?;

                // Validate terminal size bounds to prevent memory exhaustion
                if !(MIN_TERMINAL_ROWS..=MAX_TERMINAL_ROWS).contains(&rows) {
                    return Err(RunnerError::protocol(
                        "E_PROTOCOL",
                        format!(
                            "terminal rows must be between {MIN_TERMINAL_ROWS} and {MAX_TERMINAL_ROWS}"
                        ),
                        serde_json::json!({
                            "received": rows,
                            "min": MIN_TERMINAL_ROWS,
                            "max": MAX_TERMINAL_ROWS
                        }),
                    ));
                }
                if !(MIN_TERMINAL_COLS..=MAX_TERMINAL_COLS).contains(&cols) {
                    return Err(RunnerError::protocol(
                        "E_PROTOCOL",
                        format!(
                            "terminal cols must be between {MIN_TERMINAL_COLS} and {MAX_TERMINAL_COLS}"
                        ),
                        serde_json::json!({
                            "received": cols,
                            "min": MIN_TERMINAL_COLS,
                            "max": MAX_TERMINAL_COLS
                        }),
                    ));
                }

                self.master
                    .resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    })
                    .map_err(|err| RunnerError::io("E_IO", "failed to resize pty", err))?;
                let mut terminal = self.terminal.lock().map_err(|_| {
                    RunnerError::internal(
                        "E_INTERNAL",
                        "terminal lock poisoned during resize operation",
                    )
                })?;
                terminal.resize(TerminalSize { rows, cols });
                Ok(())
            }
            ActionType::Wait => Ok(()),
            ActionType::Terminate => self.terminate(),
        }
    }

    /// Read terminal output and capture a screen snapshot.
    ///
    /// Reads available PTY output up to `timeout`, processes it through the
    /// terminal emulator, and returns an observation with the current screen state.
    ///
    /// # Errors
    /// - `E_IO`: Failed to read from PTY
    /// - `E_TERMINAL_PARSE`: Output was not valid UTF-8
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

        let mut terminal = self.terminal.lock().map_err(|_| {
            RunnerError::internal("E_INTERNAL", "terminal lock poisoned during observation")
        })?;
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

    /// Wait for the child process to exit.
    ///
    /// Returns `Some(ExitStatus)` if the process exits within `timeout`,
    /// or `None` if the timeout expires.
    ///
    /// # Errors
    /// - `E_IO`: Failed to check process status
    pub fn wait_for_exit(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<portable_pty::ExitStatus>, RunnerError> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(Some(status)),
                Ok(None) => {
                    if Instant::now() >= deadline {
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

    /// Send SIGTERM to the process group.
    ///
    /// This is a best-effort termination. For graceful shutdown with
    /// fallback to SIGKILL, use [`terminate_process_group`](Self::terminate_process_group).
    ///
    /// # Errors
    /// - `E_IO`: Failed to signal process
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

    /// Gracefully terminate the process group with SIGTERM, falling back to SIGKILL.
    ///
    /// Sends SIGTERM, waits up to `grace` duration for exit, then sends SIGKILL
    /// if still alive. Returns the exit status if the process terminates.
    ///
    /// This is the recommended way to terminate a session when you need the exit status.
    ///
    /// # Errors
    /// - `E_IO`: Failed to signal or wait for process
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

    /// Get the session identifier.
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
            return Err(RunnerError::protocol(
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
    /// Explicitly close the session with proper error handling.
    ///
    /// This method provides a way to cleanly shut down the session with full
    /// error propagation, unlike the `Drop` implementation which silently ignores
    /// errors. The session is consumed by this method.
    ///
    /// Performs the following steps:
    /// 1. Flushes any buffered output to the PTY
    /// 2. Sends SIGTERM to the process group
    /// 3. Waits up to the specified grace period for graceful exit
    /// 4. Sends SIGKILL if still alive
    ///
    /// # Errors
    /// - `E_IO`: Failed to flush writer, signal process, or wait for exit
    ///
    /// # Example
    /// ```no_run
    /// # use tui_use::session::{Session, SessionConfig};
    /// # use tui_use::model::{RunId, TerminalSize};
    /// # use std::time::Duration;
    /// # fn example() -> Result<(), tui_use::runner::RunnerError> {
    /// # let config = SessionConfig {
    /// #     command: "/bin/cat".to_string(),
    /// #     args: vec![],
    /// #     cwd: None,
    /// #     size: TerminalSize::default(),
    /// #     run_id: RunId::new(),
    /// #     env: Default::default(),
    /// # };
    /// let session = Session::spawn(config)?;
    /// // ... use session ...
    /// session.close(Duration::from_millis(500))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn close(
        mut self,
        grace: Duration,
    ) -> Result<Option<portable_pty::ExitStatus>, RunnerError> {
        // Flush any buffered output before terminating
        self.writer.flush().map_err(|err| {
            RunnerError::io("E_IO", "failed to flush pty writer during close", err)
        })?;

        // Terminate and wait for exit
        self.terminate_process_group(grace)
    }

    /// Best-effort cleanup of the child process. Used by Drop.
    ///
    /// All errors are silently ignored since Drop cannot propagate errors.
    /// For controlled termination with error handling, use [`close()`](Self::close)
    /// before the Session is dropped.
    fn cleanup_process_best_effort(&mut self) {
        // Flush any buffered output (best effort, ignore errors)
        let _ = self.writer.flush();

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
