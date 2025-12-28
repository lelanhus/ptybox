//! TUI fixture: echoes individual keypresses with their byte values.
//! Used for testing keyboard input handling.

// Test fixtures require special allowances - they are not production code
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]
#![allow(clippy::indexing_slicing)]

use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = [0u8; 32];

    println!("echo_keys ready");
    stdout.flush()?;

    loop {
        let count = stdin.read(&mut buffer)?;
        if count == 0 {
            break;
        }

        let bytes = &buffer[..count];
        let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let display = String::from_utf8_lossy(bytes)
            .replace('\x1b', "ESC")
            .replace('\n', "LF")
            .replace('\r', "CR")
            .replace('\t', "TAB");

        println!("key: [{}] {}", hex.join(" "), display);
        stdout.flush()?;
    }

    Ok(())
}
