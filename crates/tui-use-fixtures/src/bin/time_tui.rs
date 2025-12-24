//! TUI fixture: prints timestamp and waits for input.
//! Used for testing timing and wait conditions.

// Test fixtures require special allowances - they are not production code
#![allow(clippy::print_stdout)] // TUI fixtures must print to terminal
#![allow(clippy::print_stderr)]

use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    println!("ts:{now}");
    io::stdout().flush().ok();

    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
}
