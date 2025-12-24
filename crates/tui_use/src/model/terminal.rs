use crate::model::SnapshotId;
use serde::{Deserialize, Serialize};

pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreenSnapshot {
    pub snapshot_version: u32,
    pub snapshot_id: SnapshotId,
    pub rows: u16,
    pub cols: u16,
    pub cursor: Cursor,
    pub alternate_screen: bool,
    pub lines: Vec<String>,
    pub cells: Option<Vec<Vec<Cell>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cell {
    pub ch: String,
    pub width: u8,
    pub style: Style,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Default,
    Ansi16(u8),
    Ansi256(u8),
    Rgb { r: u8, g: u8, b: u8 },
}
