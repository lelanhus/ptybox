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

/// Extract cell data from the screen.
fn extract_cells(screen: &vt100::Screen, rows: u16, cols: u16) -> Vec<Vec<Cell>> {
    let mut result = Vec::with_capacity(rows as usize);

    for row_idx in 0..rows {
        let mut row_cells = Vec::with_capacity(cols as usize);

        for col_idx in 0..cols {
            let vt_cell = screen.cell(row_idx, col_idx);
            if let Some(vt_cell) = vt_cell {
                // Skip wide character continuations - they're part of the previous cell
                if vt_cell.is_wide_continuation() {
                    continue;
                }

                let cell = Cell {
                    ch: vt_cell.contents().to_string(),
                    width: if vt_cell.is_wide() { 2 } else { 1 },
                    style: Style {
                        fg: convert_color(vt_cell.fgcolor()),
                        bg: convert_color(vt_cell.bgcolor()),
                        bold: vt_cell.bold(),
                        italic: vt_cell.italic(),
                        underline: vt_cell.underline(),
                        inverse: vt_cell.inverse(),
                    },
                };
                row_cells.push(cell);
            }
        }

        result.push(row_cells);
    }

    result
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
