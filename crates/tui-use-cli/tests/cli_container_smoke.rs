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

use std::process::Command;

#[test]
fn container_smoke_test() {
    if std::env::var("TUI_USE_CONTAINER_SMOKE").as_deref() != Ok("1") {
        return;
    }

    let status = Command::new("scripts/container-smoke.sh")
        .status()
        .expect("container smoke script should run");
    assert!(status.success());
}
