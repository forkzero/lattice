//! Self-update functionality for the lattice CLI.
//!
//! Downloads the latest release from GitHub, verifies the checksum,
//! and replaces the running binary.

use std::path::Path;

use colored::Colorize;
use flate2::read::GzDecoder;
use semver::Version;
use sha2::{Digest, Sha256};

const GITHUB_REPO: &str = "forkzero/lattice";

/// Compile-time target triple for the current platform.
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
pub const TARGET_TRIPLE: &str = "aarch64-apple-darwin";

#[cfg(all(target_arch = "x86_64", target_os = "macos"))]
pub const TARGET_TRIPLE: &str = "x86_64-apple-darwin";

#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
pub const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";

#[cfg(all(target_arch = "aarch64", target_os = "linux", target_env = "gnu"))]
pub const TARGET_TRIPLE: &str = "aarch64-unknown-linux-gnu";

#[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
pub const TARGET_TRIPLE: &str = "x86_64-pc-windows-msvc";

pub struct UpdateOptions {
    pub check_only: bool,
    pub force: bool,
    pub target_version: Option<String>,
}

pub enum UpdateResult {
    AlreadyUpToDate { version: Version },
    UpdateAvailable { current: Version, latest: Version },
    Updated { from: Version, to: Version },
}

#[derive(thiserror::Error, Debug)]
pub enum UpdateError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("No release found")]
    NoRelease,

    #[error("Invalid version tag: {0}")]
    InvalidVersion(String),

    #[error("No asset found for target {0}")]
    NoAsset(String),

    #[error("Checksum mismatch (expected {expected}, got {actual})")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Checksum file missing entry for {0}")]
    ChecksumNotFound(String),

    #[error("Failed to extract binary from archive: {0}")]
    Extract(String),

    #[error("Failed to replace binary: {0}")]
    Replace(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("GitHub API rate limited — try again in a few minutes")]
    RateLimited,
}

/// Build the archive filename for a given version and target.
pub fn archive_name(version: &str, target: &str) -> String {
    format!("lattice-{}-{}.tar.gz", version, target)
}

/// Parse a checksums.txt file and find the SHA-256 for a given archive filename.
pub fn parse_checksum(checksums_text: &str, archive: &str) -> Option<String> {
    for line in checksums_text.lines() {
        // Format: "<hash>  <filename>" or "<hash> <filename>"
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() == 2 {
            let filename = parts[1].trim();
            if filename == archive {
                return Some(parts[0].to_string());
            }
        }
    }
    None
}

fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION is valid semver")
}

fn build_client_with_timeout(timeout: std::time::Duration) -> Result<reqwest::Client, UpdateError> {
    Ok(reqwest::Client::builder()
        .user_agent(format!("lattice/{}", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        .build()?)
}

fn build_client() -> Result<reqwest::Client, UpdateError> {
    build_client_with_timeout(std::time::Duration::from_secs(30))
}

/// Check response status, returning RateLimited for 403.
fn check_response(status: reqwest::StatusCode, fallback: UpdateError) -> Result<(), UpdateError> {
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(UpdateError::RateLimited);
    }
    if !status.is_success() {
        return Err(fallback);
    }
    Ok(())
}

/// Fetch the latest release tag from GitHub. Returns (tag, version).
async fn fetch_latest_version(client: &reqwest::Client) -> Result<(String, Version), UpdateError> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let resp = client.get(&url).send().await?;
    check_response(resp.status(), UpdateError::NoRelease)?;

    let body: serde_json::Value = resp.json().await?;
    let tag = body["tag_name"]
        .as_str()
        .ok_or(UpdateError::NoRelease)?
        .to_string();

    let version_str = tag.strip_prefix('v').unwrap_or(&tag);
    let version =
        Version::parse(version_str).map_err(|_| UpdateError::InvalidVersion(tag.clone()))?;

    Ok((tag, version))
}

/// Fetch a specific version tag from GitHub. Returns (tag, version).
async fn fetch_specific_version(
    client: &reqwest::Client,
    target_version: &str,
) -> Result<(String, Version), UpdateError> {
    let version_str = target_version.strip_prefix('v').unwrap_or(target_version);
    let version = Version::parse(version_str)
        .map_err(|_| UpdateError::InvalidVersion(target_version.to_string()))?;
    let tag = format!("v{}", version);

    let url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        GITHUB_REPO, tag
    );
    let resp = client.get(&url).send().await?;
    check_response(resp.status(), UpdateError::NoRelease)?;

    Ok((tag, version))
}

/// Download bytes from a URL.
async fn download_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, UpdateError> {
    let resp = client.get(url).send().await?;
    check_response(resp.status(), UpdateError::NoAsset(url.to_string()))?;
    Ok(resp.bytes().await?.to_vec())
}

/// Verify SHA-256 checksum of data.
fn verify_checksum(data: &[u8], expected: &str) -> Result<(), UpdateError> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected {
        return Err(UpdateError::ChecksumMismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

/// Extract the lattice binary from a tar.gz archive into a temp file.
fn extract_binary(archive_data: &[u8], tmp_dir: &Path) -> Result<std::path::PathBuf, UpdateError> {
    let decoder = GzDecoder::new(archive_data);
    let mut archive = tar::Archive::new(decoder);

    let binary_name = if cfg!(windows) {
        "lattice.exe"
    } else {
        "lattice"
    };

    for entry in archive
        .entries()
        .map_err(|e| UpdateError::Extract(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| UpdateError::Extract(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| UpdateError::Extract(e.to_string()))?;

        // The archive has structure: lattice-{ver}-{target}/lattice
        if path.file_name().and_then(|f| f.to_str()) == Some(binary_name) {
            let dest = tmp_dir.join(binary_name);
            let mut file = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut file)?;

            // Set executable permission on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
            }

            return Ok(dest);
        }
    }

    Err(UpdateError::Extract(format!(
        "binary '{}' not found in archive",
        binary_name
    )))
}

/// Check if the running binary looks like an installed binary (not cargo run).
pub fn is_installed_binary() -> bool {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let exe_str = exe.to_string_lossy();
    // cargo run produces binaries in target/debug or target/release
    !exe_str.contains("/target/debug/")
        && !exe_str.contains("/target/release/")
        && !exe_str.contains("\\target\\debug\\")
        && !exe_str.contains("\\target\\release\\")
}

/// Run the full update flow.
pub async fn run_update(options: UpdateOptions) -> Result<UpdateResult, UpdateError> {
    let current = current_version();
    let client = build_client()?;

    // Resolve target version
    let (tag, target) = if let Some(ref v) = options.target_version {
        fetch_specific_version(&client, v).await?
    } else {
        fetch_latest_version(&client).await?
    };

    // Compare versions
    if !options.force && target <= current && options.target_version.is_none() {
        return Ok(UpdateResult::AlreadyUpToDate { version: current });
    }

    if options.check_only {
        if target > current {
            return Ok(UpdateResult::UpdateAvailable {
                current,
                latest: target,
            });
        } else {
            return Ok(UpdateResult::AlreadyUpToDate { version: current });
        }
    }

    // Download archive + checksums
    let version_str = tag.strip_prefix('v').unwrap_or(&tag);
    let archive_filename = archive_name(version_str, TARGET_TRIPLE);
    let archive_url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        GITHUB_REPO, tag, archive_filename
    );
    let checksums_url = format!(
        "https://github.com/{}/releases/download/{}/checksums.txt",
        GITHUB_REPO, tag
    );

    let (archive_result, checksums_result) = tokio::join!(
        download_bytes(&client, &archive_url),
        download_bytes(&client, &checksums_url)
    );
    let archive_data = archive_result?;
    let checksums_data = checksums_result?;
    let checksums_text = String::from_utf8_lossy(&checksums_data);

    // Verify checksum
    let expected_hash = parse_checksum(&checksums_text, &archive_filename)
        .ok_or_else(|| UpdateError::ChecksumNotFound(archive_filename.clone()))?;
    verify_checksum(&archive_data, &expected_hash)?;

    // Extract to temp dir
    let tmp_dir = tempfile::tempdir()?;
    let binary_path = extract_binary(&archive_data, tmp_dir.path())?;

    // Replace current binary
    self_replace::self_replace(&binary_path).map_err(|e| UpdateError::Replace(e.to_string()))?;

    Ok(UpdateResult::Updated {
        from: current,
        to: target,
    })
}

// --- Auto-update check (passive notification) ---

#[derive(serde::Serialize, serde::Deserialize)]
struct UpdateCheckState {
    last_checked: chrono::DateTime<chrono::Utc>,
    latest_version: String,
}

fn update_check_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join(".lattice")
            .join("update-check.json"),
    )
}

fn read_check_state(path: &std::path::Path) -> Option<UpdateCheckState> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_check_state(path: &std::path::Path, state: &UpdateCheckState) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = serde_json::to_string(state)
        .ok()
        .and_then(|json| std::fs::write(path, json).ok());
}

fn print_update_notice(latest: &Version) {
    eprintln!(
        "{}",
        format!(
            "A new version of lattice is available (v{}). Run 'lattice update' to install.",
            latest
        )
        .dimmed()
    );
}

/// Print a one-line update notice to stderr if a newer version is available.
///
/// Called after every command completes. Skips silently for `update`, `mcp`,
/// non-TTY stderr, dev builds, and when HOME is unset. Caches the check
/// result for 24 hours to avoid repeated network requests.
pub fn maybe_notify_update(command_name: Option<&str>) {
    use std::io::IsTerminal;

    // Skip for commands that handle updates themselves or use stdio protocol
    if let Some(name) = command_name
        && (name == "update" || name == "mcp")
    {
        return;
    }

    // Skip if stderr is not a terminal (piped output)
    if !std::io::stderr().is_terminal() {
        return;
    }

    // Skip dev builds
    if !is_installed_binary() {
        return;
    }

    // Need a cache path
    let cache_path = match update_check_path() {
        Some(p) => p,
        None => return,
    };

    let current = current_version();
    let now = chrono::Utc::now();

    // Check cache first
    if let Some(state) = read_check_state(&cache_path) {
        let age = now.signed_duration_since(state.last_checked);
        if age < chrono::Duration::hours(24) {
            // Cache is fresh — show notice if newer version exists
            if let Ok(latest) = Version::parse(&state.latest_version)
                && latest > current
            {
                print_update_notice(&latest);
            }
            return;
        }
    }

    // Cache is stale or missing — do a network check
    let client = match build_client_with_timeout(std::time::Duration::from_secs(5)) {
        Ok(c) => c,
        Err(_) => return,
    };

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return,
    };

    match rt.block_on(fetch_latest_version(&client)) {
        Ok((_tag, latest)) => {
            write_check_state(
                &cache_path,
                &UpdateCheckState {
                    last_checked: now,
                    latest_version: latest.to_string(),
                },
            );
            if latest > current {
                print_update_notice(&latest);
            }
        }
        Err(_) => {
            // Write timestamp anyway to suppress retries for 24h
            write_check_state(
                &cache_path,
                &UpdateCheckState {
                    last_checked: now,
                    latest_version: current.to_string(),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_triple_is_set() {
        // Just verify it compiles and returns a non-empty string
        assert!(!TARGET_TRIPLE.is_empty());
        assert!(TARGET_TRIPLE.contains('-'));
    }

    #[test]
    fn test_archive_name() {
        assert_eq!(
            archive_name("0.1.7", "aarch64-apple-darwin"),
            "lattice-0.1.7-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            archive_name("1.0.0", "x86_64-unknown-linux-gnu"),
            "lattice-1.0.0-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn test_parse_checksum_found() {
        let checksums = "\
abc123def456  lattice-0.1.7-aarch64-apple-darwin.tar.gz
789xyz000111  lattice-0.1.7-x86_64-unknown-linux-gnu.tar.gz
";
        assert_eq!(
            parse_checksum(checksums, "lattice-0.1.7-aarch64-apple-darwin.tar.gz"),
            Some("abc123def456".to_string())
        );
        assert_eq!(
            parse_checksum(checksums, "lattice-0.1.7-x86_64-unknown-linux-gnu.tar.gz"),
            Some("789xyz000111".to_string())
        );
    }

    #[test]
    fn test_parse_checksum_not_found() {
        let checksums = "abc123  lattice-0.1.7-aarch64-apple-darwin.tar.gz\n";
        assert_eq!(
            parse_checksum(checksums, "lattice-0.1.7-x86_64-pc-windows-msvc.tar.gz"),
            None
        );
    }

    #[test]
    fn test_parse_checksum_single_space() {
        let checksums = "abc123 lattice-0.1.7-aarch64-apple-darwin.tar.gz\n";
        assert_eq!(
            parse_checksum(checksums, "lattice-0.1.7-aarch64-apple-darwin.tar.gz"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn test_verify_checksum_ok() {
        let data = b"hello world";
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = format!("{:x}", hasher.finalize());
        assert!(verify_checksum(data, &hash).is_ok());
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let data = b"hello world";
        let result = verify_checksum(
            data,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            UpdateError::ChecksumMismatch { .. } => {}
            other => panic!("Expected ChecksumMismatch, got: {}", other),
        }
    }

    #[test]
    fn test_current_version_parses() {
        let v = current_version();
        // Should match CARGO_PKG_VERSION
        assert_eq!(v.to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_is_installed_binary_logic() {
        // This just tests it doesn't panic; in tests it runs from target/
        // so it should return false
        let result = is_installed_binary();
        assert!(!result, "test binary should not appear as installed");
    }

    #[test]
    fn test_update_check_state_roundtrip() {
        let state = UpdateCheckState {
            last_checked: chrono::Utc::now(),
            latest_version: "0.1.7".to_string(),
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: UpdateCheckState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.latest_version, "0.1.7");
        assert_eq!(parsed.last_checked, state.last_checked);
    }

    #[test]
    fn test_update_check_path_returns_some() {
        // HOME should be set in the test environment
        let path = update_check_path();
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.ends_with("update-check.json"));
        assert!(p.to_string_lossy().contains(".lattice"));
    }

    #[test]
    fn test_read_check_state_missing_file() {
        let path = std::path::Path::new("/tmp/nonexistent-lattice-test/update-check.json");
        assert!(read_check_state(path).is_none());
    }

    #[test]
    fn test_write_and_read_check_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        let state = UpdateCheckState {
            last_checked: chrono::Utc::now(),
            latest_version: "0.2.0".to_string(),
        };
        write_check_state(&path, &state);
        let loaded = read_check_state(&path).unwrap();
        assert_eq!(loaded.latest_version, "0.2.0");
    }
}
