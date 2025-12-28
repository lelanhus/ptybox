//! TUI fixture: echoes input with terminal size display.
//! Used for testing PTY session management.

// Test fixtures require special allowances - they are not production code
#![allow(unsafe_code)] // Required for libc::ioctl to get terminal size
#![allow(clippy::print_stdout)] // TUI fixtures must print to terminal
#![allow(clippy::print_stderr)]
#![allow(clippy::indexing_slicing)] // Safe: count is from read() which never exceeds buffer
#![allow(clippy::struct_field_names)] // Winsize follows libc C ABI naming

use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = [0u8; 1024];
    let mut input = String::new();

    render(&mut stdout, &input)?;

    loop {
        let count = stdin.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        input.push_str(&String::from_utf8_lossy(&buffer[..count]));
        render(&mut stdout, &input)?;
    }

    Ok(())
}

fn render(stdout: &mut dyn Write, input: &str) -> io::Result<()> {
    let (rows, cols) = terminal_size();
    write!(stdout, "\x1b[2J\x1b[H")?;
    writeln!(stdout, "size: {rows}x{cols}")?;
    writeln!(stdout, "input: {}", input.replace('\n', "\\n"))?;
    stdout.flush()?;
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
