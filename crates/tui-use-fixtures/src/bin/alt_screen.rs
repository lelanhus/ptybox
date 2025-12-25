//! TUI fixture: uses alternate screen buffer.
//! Used for testing alternate screen detection.

// Test fixtures require special allowances - they are not production code
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = [0u8; 32];

    // Print on main screen first
    println!("main screen");
    stdout.flush()?;

    // Wait for input to switch
    let count = stdin.read(&mut buffer)?;
    if count == 0 {
        return Ok(());
    }

    // Switch to alternate screen
    print!("\x1b[?1049h"); // Enter alternate screen
    print!("\x1b[2J\x1b[H"); // Clear and home
    println!("alternate screen");
    stdout.flush()?;

    // Wait for input to exit
    let count = stdin.read(&mut buffer)?;
    if count == 0 {
        // Restore main screen before exit
        print!("\x1b[?1049l");
        stdout.flush()?;
        return Ok(());
    }

    // Restore main screen
    print!("\x1b[?1049l");
    println!("back to main");
    stdout.flush()?;

    Ok(())
}
