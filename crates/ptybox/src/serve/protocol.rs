//! Request/response types for the stateless session UDS protocol.

use serde::{Deserialize, Serialize};

/// A single request sent over the Unix domain socket.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServeRequest {
    /// The command to execute.
    pub command: ServeCommand,
}

/// Commands that can be sent to a running session.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServeCommand {
    /// Send raw key string (e.g. `"dd"`, `"Enter"`).
    Keys {
        /// Key sequence to send.
        keys: String,
    },
    /// Type text into the terminal.
    Text {
        /// Text to type (may contain newlines).
        text: String,
    },
    /// Wait until a condition is met.
    Wait {
        /// Text the screen must contain.
        contains: Option<String>,
        /// Regex pattern the screen must match.
        matches: Option<String>,
        /// Wait timeout in milliseconds (defaults to 5000).
        timeout_ms: Option<u64>,
    },
    /// Get the current screen without sending input.
    Screen,
    /// Resize the terminal.
    Resize {
        /// Number of rows.
        rows: u16,
        /// Number of columns.
        cols: u16,
    },
    /// Terminate the session and shut down the daemon.
    Close,
}

/// Response sent back over the Unix domain socket.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServeResponse {
    /// Whether the command succeeded.
    pub ok: bool,
    /// Screen state (present on success, absent on close).
    pub screen: Option<ScreenOutput>,
    /// Error information (present on failure).
    pub error: Option<String>,
}

/// Compact screen representation optimized for token efficiency.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScreenOutput {
    /// Text lines (trailing blanks trimmed per line).
    pub lines: Vec<String>,
    /// Cursor row (0-based).
    pub cursor_row: u16,
    /// Cursor column (0-based).
    pub cursor_col: u16,
    /// Terminal height.
    pub rows: u16,
    /// Terminal width.
    pub cols: u16,
}

impl ServeResponse {
    /// Create a successful response with screen data.
    pub(crate) fn ok(screen: ScreenOutput) -> Self {
        Self {
            ok: true,
            screen: Some(screen),
            error: None,
        }
    }

    /// Create a successful response with no screen (e.g. close).
    pub(crate) fn ok_empty() -> Self {
        Self {
            ok: true,
            screen: None,
            error: None,
        }
    }

    /// Create an error response.
    pub(crate) fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            screen: None,
            error: Some(message.into()),
        }
    }
}

impl ScreenOutput {
    /// Build a compact [`ScreenOutput`] from an [`Observation`](crate::model::Observation).
    pub fn from_observation(obs: &crate::model::Observation) -> Self {
        Self {
            lines: obs.screen.lines.clone(),
            cursor_row: obs.screen.cursor.row,
            cursor_col: obs.screen.cursor.col,
            rows: obs.screen.rows,
            cols: obs.screen.cols,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keys_request() {
        let req = ServeRequest {
            command: ServeCommand::Keys {
                keys: "dd".to_string(),
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ServeRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed.command, ServeCommand::Keys { keys } if keys == "dd"));
    }

    #[test]
    fn round_trip_wait_request() {
        let req = ServeRequest {
            command: ServeCommand::Wait {
                contains: Some("hello".to_string()),
                matches: None,
                timeout_ms: Some(3000),
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ServeRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed.command,
            ServeCommand::Wait { contains: Some(t), .. } if t == "hello"
        ));
    }

    #[test]
    fn round_trip_response() {
        let resp = ServeResponse::ok(ScreenOutput {
            lines: vec!["hello".to_string()],
            cursor_row: 0,
            cursor_col: 5,
            rows: 24,
            cols: 80,
        });
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ServeResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.ok);
        assert!(parsed.screen.is_some());
    }

    #[test]
    fn round_trip_error_response() {
        let resp = ServeResponse::err("bad things happened");
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ServeResponse = serde_json::from_str(&json).unwrap();
        assert!(!parsed.ok);
        assert_eq!(parsed.error.as_deref(), Some("bad things happened"));
    }

    #[test]
    fn round_trip_all_commands() {
        let commands = vec![
            ServeCommand::Keys {
                keys: "q".to_string(),
            },
            ServeCommand::Text {
                text: "hello\n".to_string(),
            },
            ServeCommand::Wait {
                contains: None,
                matches: Some("\\d+".to_string()),
                timeout_ms: None,
            },
            ServeCommand::Screen,
            ServeCommand::Resize {
                rows: 40,
                cols: 120,
            },
            ServeCommand::Close,
        ];
        for cmd in commands {
            let req = ServeRequest { command: cmd };
            let json = serde_json::to_string(&req).unwrap();
            let _: ServeRequest = serde_json::from_str(&json).unwrap();
        }
    }
}
