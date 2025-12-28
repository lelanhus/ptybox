//! Common test helper functions.
//!
//! These utilities reduce boilerplate in integration tests by providing
//! standard implementations for temp directories and file serialization.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ptybox::model::policy::Policy;
use ptybox::model::Scenario;

/// Create a unique temporary directory for a test.
///
/// The directory name includes a timestamp to avoid collisions between
/// parallel test runs. The directory is created immediately.
///
/// # Arguments
///
/// * `prefix` - A short identifier for the test (e.g., "unicode", "resize")
///
/// # Returns
///
/// The path to the newly created directory.
///
/// # Panics
///
/// Panics if the directory cannot be created.
///
/// # Example
///
/// ```ignore
/// let dir = temp_dir("my-test");
/// // dir is something like /tmp/ptybox-my-test-1703520000000
/// ```
#[must_use]
pub fn temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    dir.push(format!("ptybox-{prefix}-{stamp}"));

    #[allow(clippy::expect_used)]
    fs::create_dir_all(&dir).expect("failed to create temp directory");

    dir
}

/// Write a policy to a JSON file.
///
/// # Arguments
///
/// * `path` - The file path to write to
/// * `policy` - The policy to serialize
///
/// # Panics
///
/// Panics if serialization or file writing fails.
///
/// # Example
///
/// ```ignore
/// let policy = PolicyBuilder::test_default(&dir).build();
/// write_policy(&dir.join("policy.json"), &policy);
/// ```
pub fn write_policy(path: &Path, policy: &Policy) {
    #[allow(clippy::expect_used)]
    let data = serde_json::to_vec_pretty(policy).expect("failed to serialize policy");

    #[allow(clippy::expect_used)]
    fs::write(path, data).expect("failed to write policy file");
}

/// Write a scenario to a JSON file.
///
/// # Arguments
///
/// * `path` - The file path to write to
/// * `scenario` - The scenario to serialize
///
/// # Panics
///
/// Panics if serialization or file writing fails.
///
/// # Example
///
/// ```ignore
/// let scenario = ScenarioBuilder::new("test", "/bin/echo").build();
/// write_scenario(&dir.join("scenario.json"), &scenario);
/// ```
pub fn write_scenario(path: &Path, scenario: &Scenario) {
    #[allow(clippy::expect_used)]
    let data = serde_json::to_vec_pretty(scenario).expect("failed to serialize scenario");

    #[allow(clippy::expect_used)]
    fs::write(path, data).expect("failed to write scenario file");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_dir_creates_unique_directories() {
        let dir1 = temp_dir("test1");
        let dir2 = temp_dir("test2");

        assert!(dir1.exists());
        assert!(dir2.exists());
        assert_ne!(dir1, dir2);

        // Cleanup
        let _ = fs::remove_dir_all(&dir1);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn temp_dir_includes_prefix() {
        let dir = temp_dir("myprefix");
        let name = dir.file_name().unwrap().to_string_lossy();

        assert!(name.contains("myprefix"));
        assert!(name.starts_with("ptybox-"));

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }
}
