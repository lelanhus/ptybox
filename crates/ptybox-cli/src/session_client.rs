//! Shared client logic for stateless session commands.
//!
//! Provides UDS connection helpers and output formatting used by
//! `keys`, `type`, `wait`, `screen`, `close`, and `sessions` commands.

use ptybox::serve::protocol::{ScreenOutput, ServeRequest, ServeResponse};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

/// Base directory for session sockets.
const SOCKET_DIR: &str = "/tmp/ptybox";

/// Socket file prefix.
const SOCKET_PREFIX: &str = "s-";

/// Compute the socket path for a given session ID.
pub fn socket_path(session_id: &str) -> PathBuf {
    PathBuf::from(SOCKET_DIR).join(format!("{SOCKET_PREFIX}{session_id}.sock"))
}

/// Return the socket directory path.
pub fn socket_dir() -> &'static Path {
    Path::new(SOCKET_DIR)
}

/// Extract session ID from a socket filename like `s-abcd1234.sock`.
pub fn session_id_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let stripped = name.strip_prefix(SOCKET_PREFIX)?;
    let id = stripped.strip_suffix(".sock")?;
    Some(id.to_string())
}

/// Connect to a session's UDS and send a request, returning the response.
pub fn send_request(session_id: &str, request: &ServeRequest) -> Result<ServeResponse, String> {
    let path = socket_path(session_id);
    if !path.exists() {
        return Err(format!(
            "session '{session_id}' not found (socket does not exist)"
        ));
    }

    let mut stream =
        UnixStream::connect(&path).map_err(|e| format!("failed to connect to session: {e}"))?;

    let json = serde_json::to_string(request).map_err(|e| format!("serialize error: {e}"))?;
    writeln!(stream, "{json}").map_err(|e| format!("write error: {e}"))?;
    stream.flush().map_err(|e| format!("flush error: {e}"))?;

    // Shut down the write half so the server knows we're done sending
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|e| format!("shutdown write error: {e}"))?;

    let mut reader = BufReader::new(&stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| format!("read error: {e}"))?;

    if response_line.trim().is_empty() {
        return Err("empty response from server".to_string());
    }

    serde_json::from_str(response_line.trim()).map_err(|e| format!("invalid response JSON: {e}"))
}

/// Format screen output for display. Returns the string to print.
pub fn format_screen_text(screen: &ScreenOutput) -> String {
    // Trim trailing empty lines for compact output
    let mut lines: Vec<&str> = screen.lines.iter().map(String::as_str).collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Check if a session is alive by attempting a brief connection.
pub fn is_session_alive(session_id: &str) -> bool {
    let path = socket_path(session_id);
    UnixStream::connect(&path).is_ok()
}
