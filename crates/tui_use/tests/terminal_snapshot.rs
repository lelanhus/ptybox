// Test module - relaxed lint rules
#![allow(clippy::default_trait_access)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::inefficient_to_string)]
#![allow(clippy::panic)]
#![allow(clippy::manual_assert)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::cast_possible_truncation)]
#![allow(missing_docs)]

use tui_use::model::TerminalSize;
use tui_use::terminal::Terminal;

#[test]
fn terminal_snapshot_includes_cursor_and_alt_screen() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 10 });
    terminal.process_bytes(b"\x1b[?1049h");
    terminal.process_bytes(b"hi");
    terminal.process_bytes(b"\x1b[2;3H");
    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    assert!(snapshot.alternate_screen);
    assert!(snapshot.cursor.row > 0);
    assert!(snapshot.cursor.col > 0);
}

#[test]
fn terminal_snapshot_preserves_unicode_and_wide_chars() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes("🙂漢字".as_bytes());
    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    let joined = snapshot.lines.join("\n");
    assert!(joined.contains("🙂"));
    assert!(joined.contains("漢字"));
}
