//! TUI fixture: exits with a specified exit code.
//! Used for testing exit code handling.
//!
//! Usage: `exit_code <code>`

// Test fixtures require special allowances - they are not production code
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]
#![allow(clippy::exit)]

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let code: i32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    println!("exiting with code {code}");

    process::exit(code);
}
