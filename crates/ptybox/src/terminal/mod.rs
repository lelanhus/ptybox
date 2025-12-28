//! Terminal emulation and screen snapshot capture.
//!
//! This module wraps the `vt100` crate to provide ANSI/VT100 terminal emulation,
//! processing raw byte streams from a PTY and producing canonical screen snapshots
//! that can be used for assertions, replay comparison, and artifact storage.
//!
//! # Key Types
//!
//! - [`Terminal`] - Terminal emulator that processes ANSI escape sequences
//! - [`ScreenSnapshot`] - Immutable capture of terminal state (from `crate::model`)
//! - [`Cell`] - Individual cell with character and styling (from `crate::model`)
//!
//! # Key Operations
//!
//! - [`Terminal::new`] - Create a new terminal with specified dimensions
//! - [`Terminal::resize`] - Change terminal dimensions
//! - [`Terminal::process_bytes`] - Feed raw PTY output through the emulator
//! - [`Terminal::snapshot`] - Capture current screen state without cell styling
//! - [`Terminal::snapshot_with_cells`] - Capture screen state with optional cell styling
//!
//! # Example
//!
//! ```
//! use ptybox::terminal::Terminal;
//! use ptybox::model::TerminalSize;
//!
//! # fn example() -> Result<(), ptybox::runner::RunnerError> {
//! // Create a terminal with default size
//! let mut terminal = Terminal::new(TerminalSize { rows: 24, cols: 80 });
//!
//! // Process some terminal output (including ANSI escape sequences)
//! terminal.process_bytes(b"Hello, \x1b[1mBold\x1b[0m World!\r\n");
//!
//! // Capture the current screen state
//! let snapshot = terminal.snapshot()?;
//! assert!(snapshot.lines[0].contains("Hello"));
//! # Ok(())
//! # }
//! ```
//!
//! # Terminal Features
//!
//! The underlying `vt100` parser supports:
//! - Standard ANSI escape sequences (cursor movement, clearing, etc.)
//! - Text attributes (bold, italic, underline, inverse)
//! - 16-color, 256-color, and true color (RGB) support
//! - Alternate screen buffer detection
//! - Wide character (CJK) handling

use crate::model::{Cell, Color, Cursor, ScreenSnapshot, SnapshotId, Style, TerminalSize};
use crate::runner::RunnerError;
use vt100::Parser;

/// Terminal emulator wrapper using vt100.
pub struct Terminal {
    parser: Parser,
}

impl Terminal {
    /// Create a new terminal with the given size.
    pub fn new(size: TerminalSize) -> Self {
        Self {
            parser: Parser::new(size.rows, size.cols, 0),
        }
    }

    /// Resize the terminal.
    pub fn resize(&mut self, size: TerminalSize) {
        self.parser.set_size(size.rows, size.cols);
    }

    /// Process incoming bytes.
    pub fn process_bytes(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    /// Take a snapshot of the terminal screen without cell styling.
    pub fn snapshot(&self) -> Result<ScreenSnapshot, RunnerError> {
        self.snapshot_with_cells(false)
    }

    /// Take a snapshot of the terminal screen, optionally including cell styling.
    pub fn snapshot_with_cells(&self, include_cells: bool) -> Result<ScreenSnapshot, RunnerError> {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let lines: Vec<String> = screen.rows(0, cols).collect();
        let (row, col) = screen.cursor_position();
        let cursor = Cursor {
            row,
            col,
            visible: !screen.hide_cursor(),
        };

        let cells = if include_cells {
            Some(extract_cells(screen, rows, cols))
        } else {
            None
        };

        Ok(ScreenSnapshot {
            snapshot_version: 1,
            snapshot_id: SnapshotId::new(),
            rows,
            cols,
            cursor,
            alternate_screen: screen.alternate_screen(),
            lines,
            cells,
        })
    }
}

/// Extract cell data from the screen using iterator chains.
fn extract_cells(screen: &vt100::Screen, rows: u16, cols: u16) -> Vec<Vec<Cell>> {
    (0..rows)
        .map(|row_idx| {
            (0..cols)
                .filter_map(|col_idx| {
                    screen.cell(row_idx, col_idx).and_then(|vt_cell| {
                        // Skip wide character continuations - they're part of the previous cell
                        if vt_cell.is_wide_continuation() {
                            return None;
                        }
                        Some(vt_cell_to_cell(vt_cell))
                    })
                })
                .collect()
        })
        .collect()
}

/// Convert a vt100 cell to our Cell model.
fn vt_cell_to_cell(vt_cell: &vt100::Cell) -> Cell {
    Cell {
        ch: vt_cell.contents().clone(),
        width: if vt_cell.is_wide() { 2 } else { 1 },
        style: Style {
            fg: convert_color(vt_cell.fgcolor()),
            bg: convert_color(vt_cell.bgcolor()),
            bold: vt_cell.bold(),
            italic: vt_cell.italic(),
            underline: vt_cell.underline(),
            inverse: vt_cell.inverse(),
        },
    }
}

/// Convert vt100 color to our model color.
fn convert_color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Default,
        vt100::Color::Idx(n) if n < 16 => Color::Ansi16(n),
        vt100::Color::Idx(n) => Color::Ansi256(n),
        vt100::Color::Rgb(r, g, b) => Color::Rgb { r, g, b },
    }
}
