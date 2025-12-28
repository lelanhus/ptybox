//! TUI fixture: displays terminal size and updates on resize.
//! Used for testing resize actions.
//!
//! Usage:
//!   ptybox-show-size         # Interactive mode: prints on each input
//!   ptybox-show-size once    # Prints once and exits

// Test fixtures require special allowances - they are not production code
#![allow(unsafe_code)]
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]
#![allow(clippy::struct_field_names)]

use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let once_mode = args.get(1).is_some_and(|arg| arg == "once");

    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = [0u8; 32];

    // Initial display
    let (rows, cols) = terminal_size();
    println!("size: {rows} rows x {cols} cols");
    stdout.flush()?;

    // If "once" mode, exit immediately after printing
    if once_mode {
        return Ok(());
    }

    // Wait for input (allows resize events to be processed)
    loop {
        let count = stdin.read(&mut buffer)?;
        if count == 0 {
            break;
        }

        // On any input, show current size
        let (rows, cols) = terminal_size();
        println!("size: {rows} rows x {cols} cols");
        stdout.flush()?;
    }

    Ok(())
}

fn terminal_size() -> (u16, u16) {
    #[repr(C)]
    struct Winsize {
        ws_row: u16,
        ws_col: u16,
        ws_xpixel: u16,
        ws_ypixel: u16,
    }

    let mut size = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let result = unsafe { libc::ioctl(0, libc::TIOCGWINSZ, &mut size) };
    if result == 0 {
        (size.ws_row, size.ws_col)
    } else {
        (0, 0)
    }
}
