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

use tui_use::model::{Color, TerminalSize};
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

// ============================================================================
// Cell extraction tests
// ============================================================================

#[test]
fn snapshot_captures_cell_content() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"Hello");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    assert!(!cells.is_empty(), "should have cell rows");
    assert!(!cells[0].is_empty(), "first row should have cells");

    // Check that first 5 cells contain "Hello"
    let content: String = cells[0].iter().take(5).map(|c| c.ch.as_str()).collect();
    assert_eq!(content, "Hello");
}

#[test]
fn snapshot_without_cells_has_none() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"test");
    let snapshot = terminal
        .snapshot_with_cells(false)
        .expect("snapshot should succeed");

    assert!(
        snapshot.cells.is_none(),
        "cells should be None when not requested"
    );
}

#[test]
fn snapshot_captures_foreground_color() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 31 = red foreground (ANSI color index 1)
    terminal.process_bytes(b"\x1b[31mR");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let cell = &cells[0][0];
    assert_eq!(cell.ch, "R");
    assert!(
        matches!(cell.style.fg, Color::Ansi16(1)),
        "foreground should be ANSI red (1), got {:?}",
        cell.style.fg
    );
}

#[test]
fn snapshot_captures_background_color() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 44 = blue background (ANSI color index 4)
    terminal.process_bytes(b"\x1b[44mB");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let cell = &cells[0][0];
    assert_eq!(cell.ch, "B");
    assert!(
        matches!(cell.style.bg, Color::Ansi16(4)),
        "background should be ANSI blue (4), got {:?}",
        cell.style.bg
    );
}

#[test]
fn snapshot_captures_bold_attribute() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 1 = bold
    terminal.process_bytes(b"\x1b[1mB");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let cell = &cells[0][0];
    assert!(cell.style.bold, "cell should be bold");
}

#[test]
fn snapshot_captures_italic_attribute() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 3 = italic
    terminal.process_bytes(b"\x1b[3mI");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let cell = &cells[0][0];
    assert!(cell.style.italic, "cell should be italic");
}

#[test]
fn snapshot_captures_underline_attribute() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 4 = underline
    terminal.process_bytes(b"\x1b[4mU");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let cell = &cells[0][0];
    assert!(cell.style.underline, "cell should be underlined");
}

#[test]
fn snapshot_captures_inverse_attribute() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 7 = inverse
    terminal.process_bytes(b"\x1b[7mV");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let cell = &cells[0][0];
    assert!(cell.style.inverse, "cell should be inverse");
}

#[test]
fn snapshot_handles_ansi_16_colors() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 30-37 are colors 0-7, SGR 90-97 are colors 8-15
    // 32 = green (2), 91 = bright red (9)
    terminal.process_bytes(b"\x1b[32mG\x1b[91mR");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");

    assert!(
        matches!(cells[0][0].style.fg, Color::Ansi16(2)),
        "first cell should be green (2), got {:?}",
        cells[0][0].style.fg
    );
    assert!(
        matches!(cells[0][1].style.fg, Color::Ansi16(9)),
        "second cell should be bright red (9), got {:?}",
        cells[0][1].style.fg
    );
}

#[test]
fn snapshot_handles_ansi_256_colors() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 38;5;N = 256-color foreground
    // SGR 48;5;N = 256-color background
    terminal.process_bytes(b"\x1b[38;5;123mF\x1b[48;5;200mB");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");

    assert!(
        matches!(cells[0][0].style.fg, Color::Ansi256(123)),
        "foreground should be 256-color 123, got {:?}",
        cells[0][0].style.fg
    );
    assert!(
        matches!(cells[0][1].style.bg, Color::Ansi256(200)),
        "background should be 256-color 200, got {:?}",
        cells[0][1].style.bg
    );
}

#[test]
fn snapshot_handles_rgb_colors() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // SGR 38;2;R;G;B = RGB foreground
    // SGR 48;2;R;G;B = RGB background
    terminal.process_bytes(b"\x1b[38;2;255;128;64mF\x1b[48;2;10;20;30mB");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");

    match &cells[0][0].style.fg {
        Color::Rgb { r, g, b } => {
            assert_eq!(*r, 255);
            assert_eq!(*g, 128);
            assert_eq!(*b, 64);
        }
        other => panic!("expected RGB foreground, got {:?}", other),
    }

    match &cells[0][1].style.bg {
        Color::Rgb { r, g, b } => {
            assert_eq!(*r, 10);
            assert_eq!(*g, 20);
            assert_eq!(*b, 30);
        }
        other => panic!("expected RGB background, got {:?}", other),
    }
}

#[test]
fn snapshot_handles_wide_characters() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // Wide characters like emoji and CJK take 2 cells
    terminal.process_bytes("🙂AB".as_bytes());
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let row = &cells[0];

    // Find the emoji cell
    let emoji_cell = row.iter().find(|c| c.ch == "🙂");
    assert!(emoji_cell.is_some(), "should find emoji cell");
    assert_eq!(emoji_cell.unwrap().width, 2, "emoji should have width 2");

    // Continuation cells should be skipped, so A should follow directly
    let chars: Vec<&str> = row.iter().map(|c| c.ch.as_str()).collect();
    assert!(chars.contains(&"🙂"), "should contain emoji");
    assert!(chars.contains(&"A"), "should contain A");
    assert!(chars.contains(&"B"), "should contain B");
}

#[test]
fn snapshot_handles_cjk_wide_characters() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes("漢字".as_bytes());
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let row = &cells[0];

    // Both characters should have width 2
    let cjk_cells: Vec<_> = row
        .iter()
        .filter(|c| !c.ch.is_empty() && !c.ch.trim().is_empty())
        .collect();
    assert!(!cjk_cells.is_empty(), "should have CJK cells");

    for cell in cjk_cells {
        if cell.ch == "漢" || cell.ch == "字" {
            assert_eq!(
                cell.width, 2,
                "CJK character {} should have width 2",
                cell.ch
            );
        }
    }
}

#[test]
fn snapshot_default_style_is_correct() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"X");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let cell = &cells[0][0];

    assert!(
        matches!(cell.style.fg, Color::Default),
        "default fg should be Default"
    );
    assert!(
        matches!(cell.style.bg, Color::Default),
        "default bg should be Default"
    );
    assert!(!cell.style.bold, "default should not be bold");
    assert!(!cell.style.italic, "default should not be italic");
    assert!(!cell.style.underline, "default should not be underlined");
    assert!(!cell.style.inverse, "default should not be inverse");
}

#[test]
fn snapshot_combined_attributes() {
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // Bold + underline + red fg + blue bg
    terminal.process_bytes(b"\x1b[1;4;31;44mX");
    let snapshot = terminal
        .snapshot_with_cells(true)
        .expect("snapshot should succeed");

    let cells = snapshot.cells.expect("cells should be present");
    let cell = &cells[0][0];

    assert!(cell.style.bold, "should be bold");
    assert!(cell.style.underline, "should be underlined");
    assert!(
        matches!(cell.style.fg, Color::Ansi16(1)),
        "fg should be red"
    );
    assert!(
        matches!(cell.style.bg, Color::Ansi16(4)),
        "bg should be blue"
    );
}

// ============================================================================
// Edge case tests - malformed input handling
// ============================================================================

#[test]
fn terminal_handles_incomplete_escape_sequence() {
    // Incomplete escape sequence should be handled gracefully
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"Hello\x1b["); // Incomplete CSI sequence
    terminal.process_bytes(b"World"); // Continue with more text

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    // Terminal should not crash and should capture some content
    assert!(!snapshot.lines.is_empty());
}

#[test]
fn terminal_handles_unknown_escape_sequence() {
    // Unknown CSI sequence should be handled gracefully
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"\x1b[999zHello"); // Unknown CSI sequence followed by text

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    let joined = snapshot.lines.join("");
    // The "Hello" text should still be captured
    assert!(
        joined.contains("Hello"),
        "text after unknown sequence should be captured"
    );
}

#[test]
fn terminal_handles_null_bytes() {
    // Null bytes in output should be handled gracefully
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"Hello\x00World");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    // Should not crash and should capture content
    assert!(!snapshot.lines.is_empty());
}

#[test]
fn terminal_handles_very_long_escape_sequence() {
    // Very long escape sequence parameters should be handled
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    // Create a CSI sequence with many parameters
    let mut seq = b"\x1b[".to_vec();
    for i in 0..100 {
        if i > 0 {
            seq.push(b';');
        }
        seq.extend_from_slice(i.to_string().as_bytes());
    }
    seq.push(b'm');
    terminal.process_bytes(&seq);
    terminal.process_bytes(b"Text");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    // Should not crash and should capture content
    assert!(!snapshot.lines.is_empty());
}

#[test]
fn terminal_handles_bell_character() {
    // Bell character (BEL, 0x07) should be handled gracefully
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"Hello\x07World");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    let joined = snapshot.lines.join("");
    assert!(joined.contains("Hello"));
    assert!(joined.contains("World"));
}

#[test]
fn terminal_handles_carriage_return_only() {
    // CR without LF should return to start of line
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"AAAAA\rBB");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    let first_line = &snapshot.lines[0];
    // "BB" should overwrite first two "A"s
    assert!(
        first_line.starts_with("BB"),
        "CR should return to start of line"
    );
}

#[test]
fn terminal_handles_multiple_incomplete_sequences() {
    // Multiple incomplete escape sequences in succession
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"\x1b[\x1b[\x1b[mHello");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    // Should not crash and should capture text
    assert!(!snapshot.lines.is_empty());
}

#[test]
fn terminal_handles_interleaved_valid_invalid_sequences() {
    // Mix of valid and invalid sequences
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"\x1b[31mRed\x1b[999zInvalid\x1b[0mNormal");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    let joined = snapshot.lines.join("");
    assert!(joined.contains("Red"), "valid red text should be captured");
    assert!(
        joined.contains("Normal"),
        "text after reset should be captured"
    );
}

#[test]
fn terminal_handles_escape_at_end_of_input() {
    // Escape character as the very last byte
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"Hello\x1b");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    let joined = snapshot.lines.join("");
    assert!(
        joined.contains("Hello"),
        "text before escape should be captured"
    );
}

#[test]
fn terminal_handles_csi_at_end_of_input() {
    // CSI sequence start as the very last bytes
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"Hello\x1b[");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    let joined = snapshot.lines.join("");
    assert!(
        joined.contains("Hello"),
        "text before CSI should be captured"
    );
}

#[test]
fn terminal_handles_backspace_character() {
    // Backspace should move cursor back
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"ABC\x08X");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    let first_line = &snapshot.lines[0];
    // X should overwrite C
    assert!(
        first_line.contains("ABX"),
        "backspace should allow overwrite, got: {}",
        first_line
    );
}

#[test]
fn terminal_handles_tab_character() {
    // Tab should advance cursor
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"A\tB");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    // Should not crash and should contain both characters
    let joined = snapshot.lines.join("");
    assert!(joined.contains('A'), "should contain A");
    assert!(joined.contains('B'), "should contain B");
}

#[test]
fn terminal_handles_form_feed() {
    // Form feed (0x0C) should be handled gracefully
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"Hello\x0cWorld");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    // Should not crash
    assert!(!snapshot.lines.is_empty());
}

#[test]
fn terminal_handles_vertical_tab() {
    // Vertical tab (0x0B) should be handled gracefully
    let mut terminal = Terminal::new(TerminalSize { rows: 5, cols: 20 });
    terminal.process_bytes(b"Hello\x0bWorld");

    let snapshot = terminal.snapshot().expect("snapshot should succeed");
    // Should not crash
    assert!(!snapshot.lines.is_empty());
}
