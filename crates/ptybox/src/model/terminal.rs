use crate::model::SnapshotId;
use serde::{Deserialize, Serialize};

/// Version of the screen snapshot format.
pub const SNAPSHOT_VERSION: u32 = 1;

/// Terminal dimensions in rows and columns.
///
/// Default is 24 rows by 80 columns (standard VT100 size).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSize {
    /// Number of rows (height).
    pub rows: u16,
    /// Number of columns (width).
    pub cols: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

/// Cursor position and visibility state.
///
/// Coordinates are 0-based (row 0 is top, col 0 is left).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    /// Row position (0-based, 0 is top).
    pub row: u16,
    /// Column position (0-based, 0 is left).
    pub col: u16,
    /// Whether the cursor is visible.
    pub visible: bool,
}

/// Canonical snapshot of terminal state at a point in time.
///
/// Contains normalized text lines and optional cell-level style data.
/// The `lines` field is the primary way to verify screen content in assertions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenSnapshot {
    /// Format version for compatibility checking.
    pub snapshot_version: u32,
    /// Unique identifier for this snapshot.
    pub snapshot_id: SnapshotId,
    /// Number of rows in the terminal.
    pub rows: u16,
    /// Number of columns in the terminal.
    pub cols: u16,
    /// Current cursor state.
    pub cursor: Cursor,
    /// Whether the terminal is in alternate screen mode.
    pub alternate_screen: bool,
    /// Normalized text lines (one per row, trailing spaces trimmed).
    pub lines: Vec<String>,
    /// Optional cell-level data with styling (only when style tracking is enabled).
    pub cells: Option<Vec<Vec<Cell>>>,
}

/// Single terminal cell with character and styling.
///
/// Used when detailed style information is needed beyond plain text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// Character or grapheme cluster in this cell.
    pub ch: String,
    /// Display width (typically 1 or 2 for wide characters).
    pub width: u8,
    /// Visual style applied to this cell.
    pub style: Style,
}

/// Terminal cell styling attributes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Style {
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
    /// Underlined text.
    pub underline: bool,
    /// Inverse/reverse video mode.
    pub inverse: bool,
}

/// Terminal color representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    /// Terminal default color.
    Default,
    /// 16-color ANSI palette (0-15).
    Ansi16(u8),
    /// 256-color extended palette (0-255).
    Ansi256(u8),
    /// 24-bit true color RGB.
    Rgb { r: u8, g: u8, b: u8 },
}
