use crate::model::{Cursor, ScreenSnapshot, SnapshotId, TerminalSize};
use crate::runner::RunnerError;
use vt100::Parser;

pub struct Terminal {
    parser: Parser,
}

impl Terminal {
    pub fn new(size: TerminalSize) -> Self {
        Self {
            parser: Parser::new(size.rows, size.cols, 0),
        }
    }

    pub fn resize(&mut self, size: TerminalSize) {
        self.parser.set_size(size.rows, size.cols);
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn snapshot(&self) -> Result<ScreenSnapshot, RunnerError> {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let lines: Vec<String> = screen.rows(0, cols).collect();
        let (row, col) = screen.cursor_position();
        let cursor = Cursor {
            row,
            col,
            visible: !screen.hide_cursor(),
        };
        Ok(ScreenSnapshot {
            snapshot_version: 1,
            snapshot_id: SnapshotId::new(),
            rows,
            cols,
            cursor,
            alternate_screen: screen.alternate_screen(),
            lines,
            cells: None,
        })
    }
}
