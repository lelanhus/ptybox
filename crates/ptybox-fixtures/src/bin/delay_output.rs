//! TUI fixture: outputs text after a specified delay.
//! Used for testing wait conditions.
//!
//! Usage: `delay_output <delay_ms> <message>`

// Test fixtures require special allowances - they are not production code
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();

    let delay_ms: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100);
    let message = args.get(2).map(String::as_str).unwrap_or("delayed");

    println!("waiting...");
    io::stdout().flush().ok();

    thread::sleep(Duration::from_millis(delay_ms));

    println!("{message}");
    io::stdout().flush().ok();
}
