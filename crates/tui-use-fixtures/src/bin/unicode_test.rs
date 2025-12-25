//! TUI fixture: prints Unicode including wide characters, emoji, and CJK.
//! Used for testing Unicode handling in terminal snapshots.

// Test fixtures require special allowances - they are not production code
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use std::io::{self, Write};

fn main() {
    // ASCII
    println!("ASCII: Hello, World!");

    // Accented characters (Latin Extended)
    println!("Accents: café résumé naïve");

    // Wide characters - CJK
    println!("CJK: 你好世界 こんにちは 안녕하세요");

    // Emoji (varying widths)
    println!("Emoji: 🎉 🚀 ✨ 🔥 💡 ⭐");

    // Box drawing characters
    println!("Box: ┌──────┐");
    println!("     │ test │");
    println!("     └──────┘");

    // Combining characters
    println!("Combining: e\u{0301} n\u{0303} o\u{0308}"); // é ñ ö

    // Right-to-left (Hebrew/Arabic)
    println!("RTL: שלום مرحبا");

    // Mathematical symbols
    println!("Math: ∑ ∏ ∫ √ ∞ ≠ ≤ ≥");

    // Currency symbols
    println!("Currency: $ € £ ¥ ₹ ₿");

    io::stdout().flush().ok();
}
