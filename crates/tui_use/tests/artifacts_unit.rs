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

//! Artifacts module unit tests
//!
//! Tests the artifact writing and checksum functionality.

use std::fs;
use std::path::PathBuf;
use tui_use::artifacts::{ArtifactsWriter, ArtifactsWriterConfig};
use tui_use::model::{
    Cursor, NormalizationRecord, NormalizationSource, Policy, RunId, ScreenSnapshot, SnapshotId,
    NORMALIZATION_VERSION, SNAPSHOT_VERSION,
};
use tui_use::runner::ErrorCode;

fn temp_artifacts_dir() -> PathBuf {
    // Include thread ID for test isolation when running tests in parallel
    let thread_id = format!("{:?}", std::thread::current().id());
    // Extract numeric part from ThreadId format "ThreadId(N)"
    let thread_num = thread_id
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();

    let dir = std::env::temp_dir().join(format!(
        "tui-use-artifacts-test-{}-{}-{}",
        std::process::id(),
        thread_num,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // Clean up any existing dir
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn cleanup_dir(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

// =============================================================================
// Directory Creation Tests
// =============================================================================

#[test]
fn artifacts_writer_creates_directory() {
    let dir = temp_artifacts_dir();
    assert!(!dir.exists(), "Directory should not exist before test");

    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let result = ArtifactsWriter::new(run_id, config);
    assert!(result.is_ok(), "Should create writer: {:?}", result.err());
    assert!(dir.exists(), "Directory should be created");
    assert!(
        dir.join("transcript.log").exists(),
        "transcript.log should be created"
    );
    assert!(
        dir.join("events.jsonl").exists(),
        "events.jsonl should be created"
    );

    cleanup_dir(&dir);
}

#[test]
fn artifacts_writer_fails_if_exists_and_no_overwrite() {
    let dir = temp_artifacts_dir();
    fs::create_dir_all(&dir).expect("Failed to create test dir");

    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let result = ArtifactsWriter::new(run_id, config);
    assert!(result.is_err(), "Should fail when dir exists");
    match result {
        Err(err) => assert_eq!(err.code, ErrorCode::PolicyDenied),
        Ok(_) => panic!("Should have failed"),
    }

    cleanup_dir(&dir);
}

#[test]
fn artifacts_writer_allows_overwrite_if_enabled() {
    let dir = temp_artifacts_dir();
    fs::create_dir_all(&dir).expect("Failed to create test dir");

    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: true,
    };
    let run_id = RunId::new();

    let result = ArtifactsWriter::new(run_id, config);
    assert!(
        result.is_ok(),
        "Should succeed with overwrite=true: {:?}",
        result.err()
    );

    cleanup_dir(&dir);
}

// =============================================================================
// Write Policy Tests
// =============================================================================

#[test]
fn artifacts_write_policy() {
    let dir = temp_artifacts_dir();
    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let mut writer = ArtifactsWriter::new(run_id, config).expect("Failed to create writer");

    let policy = Policy::default();
    let result = writer.write_policy(&policy);
    assert!(result.is_ok(), "Should write policy: {:?}", result.err());

    let policy_path = dir.join("policy.json");
    assert!(policy_path.exists(), "policy.json should exist");

    // Verify content is valid JSON
    let content = fs::read_to_string(&policy_path).expect("Failed to read policy");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("Should be valid JSON");
    assert!(parsed.is_object(), "Should be a JSON object");

    cleanup_dir(&dir);
}

// =============================================================================
// Write Snapshot Tests
// =============================================================================

#[test]
fn artifacts_write_snapshot() {
    let dir = temp_artifacts_dir();
    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let mut writer = ArtifactsWriter::new(run_id, config).expect("Failed to create writer");

    let snapshot = ScreenSnapshot {
        snapshot_version: SNAPSHOT_VERSION,
        snapshot_id: SnapshotId::new(),
        rows: 24,
        cols: 80,
        cursor: Cursor {
            row: 0,
            col: 0,
            visible: true,
        },
        alternate_screen: false,
        lines: vec!["Line 1".to_string(), "Line 2".to_string()],
        cells: None,
    };

    let result = writer.write_snapshot(&snapshot);
    assert!(result.is_ok(), "Should write snapshot: {:?}", result.err());

    let snapshot_path = dir.join("snapshots/000001.json");
    assert!(snapshot_path.exists(), "Snapshot file should exist");

    // Verify content
    let content = fs::read_to_string(&snapshot_path).expect("Failed to read snapshot");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("Should be valid JSON");
    let lines = parsed.get("lines").expect("Should have lines");
    assert_eq!(lines.as_array().unwrap().len(), 2);

    cleanup_dir(&dir);
}

#[test]
fn artifacts_write_multiple_snapshots_numbered() {
    let dir = temp_artifacts_dir();
    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let mut writer = ArtifactsWriter::new(run_id, config).expect("Failed to create writer");

    let snapshot = ScreenSnapshot {
        snapshot_version: SNAPSHOT_VERSION,
        snapshot_id: SnapshotId::new(),
        rows: 24,
        cols: 80,
        cursor: Cursor {
            row: 0,
            col: 0,
            visible: true,
        },
        alternate_screen: false,
        lines: vec!["test".to_string()],
        cells: None,
    };

    for _ in 0..3 {
        writer
            .write_snapshot(&snapshot)
            .expect("Failed to write snapshot");
    }

    assert!(dir.join("snapshots/000001.json").exists());
    assert!(dir.join("snapshots/000002.json").exists());
    assert!(dir.join("snapshots/000003.json").exists());

    cleanup_dir(&dir);
}

// =============================================================================
// Write Transcript Tests
// =============================================================================

#[test]
fn artifacts_write_transcript() {
    let dir = temp_artifacts_dir();
    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let mut writer = ArtifactsWriter::new(run_id, config).expect("Failed to create writer");

    let result = writer.write_transcript("Hello, World!\n");
    assert!(
        result.is_ok(),
        "Should write transcript: {:?}",
        result.err()
    );

    let result = writer.write_transcript("Second line\n");
    assert!(result.is_ok(), "Should append transcript");

    let transcript_path = dir.join("transcript.log");
    let content = fs::read_to_string(&transcript_path).expect("Failed to read transcript");
    assert_eq!(content, "Hello, World!\nSecond line\n");

    cleanup_dir(&dir);
}

// =============================================================================
// Checksum Tests
// =============================================================================

/// Compute FNV-1a hash (64-bit) for checksum verification
fn compute_fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;
    let mut hash: u64 = FNV_OFFSET_BASIS;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[test]
fn artifacts_checksum_correctness() {
    let dir = temp_artifacts_dir();
    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let mut writer = ArtifactsWriter::new(run_id, config).expect("Failed to create writer");

    // Write some data
    let policy = Policy::default();
    writer
        .write_policy(&policy)
        .expect("Failed to write policy");

    // Read checksums
    let checksums_path = dir.join("checksums.json");
    assert!(checksums_path.exists(), "checksums.json should exist");

    let content = fs::read_to_string(&checksums_path).expect("Failed to read checksums");
    let checksums: serde_json::Value =
        serde_json::from_str(&content).expect("Should be valid JSON");

    // Verify policy.json has a checksum
    let policy_checksum = checksums.get("policy.json");
    assert!(
        policy_checksum.is_some(),
        "Should have checksum for policy.json"
    );
    assert!(
        policy_checksum.unwrap().is_string(),
        "Checksum should be a string"
    );

    // Verify checksum format (16 hex characters)
    let checksum_str = policy_checksum.unwrap().as_str().unwrap();
    assert_eq!(checksum_str.len(), 16, "Checksum should be 16 hex chars");
    assert!(
        checksum_str.chars().all(|c| c.is_ascii_hexdigit()),
        "Checksum should be hex"
    );

    // Read policy.json content and compute FNV-1a hash to verify checksum correctness
    let policy_path = dir.join("policy.json");
    let policy_content = fs::read(&policy_path).expect("Failed to read policy.json");

    // Compute FNV-1a hash (64-bit) and verify it matches stored checksum
    let computed_checksum = format!("{:016x}", compute_fnv1a_hash(&policy_content));

    assert_eq!(
        checksum_str, computed_checksum,
        "Stored checksum should match computed FNV-1a hash"
    );

    cleanup_dir(&dir);
}

#[test]
fn artifacts_checksum_updates_on_write() {
    let dir = temp_artifacts_dir();
    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let mut writer = ArtifactsWriter::new(run_id, config).expect("Failed to create writer");

    // Write normalization first
    let record = NormalizationRecord {
        normalization_version: NORMALIZATION_VERSION,
        filters: Vec::new(),
        strict: false,
        source: NormalizationSource::None,
        rules: Vec::new(),
    };
    writer
        .write_normalization(&record)
        .expect("Failed to write normalization");

    // Read checksums after first write
    let checksums_path = dir.join("checksums.json");
    let content1 = fs::read_to_string(&checksums_path).expect("Failed to read checksums");
    let checksums1: serde_json::Value =
        serde_json::from_str(&content1).expect("Should be valid JSON");

    // Write policy
    let policy = Policy::default();
    writer
        .write_policy(&policy)
        .expect("Failed to write policy");

    // Read checksums after second write
    let content2 = fs::read_to_string(&checksums_path).expect("Failed to read checksums");
    let checksums2: serde_json::Value =
        serde_json::from_str(&content2).expect("Should be valid JSON");

    // Should have more entries now
    assert!(
        checksums2.as_object().unwrap().len() > checksums1.as_object().unwrap().len(),
        "Checksums should grow as files are written"
    );

    cleanup_dir(&dir);
}

#[test]
fn artifacts_checksum_not_self_referential() {
    let dir = temp_artifacts_dir();
    let config = ArtifactsWriterConfig {
        dir: dir.clone(),
        overwrite: false,
    };
    let run_id = RunId::new();

    let mut writer = ArtifactsWriter::new(run_id, config).expect("Failed to create writer");

    let policy = Policy::default();
    writer
        .write_policy(&policy)
        .expect("Failed to write policy");

    let checksums_path = dir.join("checksums.json");
    let content = fs::read_to_string(&checksums_path).expect("Failed to read checksums");
    let checksums: serde_json::Value =
        serde_json::from_str(&content).expect("Should be valid JSON");

    // checksums.json should NOT contain a checksum for itself
    assert!(
        checksums.get("checksums.json").is_none(),
        "checksums.json should not contain itself"
    );

    cleanup_dir(&dir);
}
