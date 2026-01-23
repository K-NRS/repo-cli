use anyhow::{anyhow, Result};
use semver::Version;

/// Current version of the repo-cli binary
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Parse a version string (with or without 'v' prefix)
pub fn parse_version(version_str: &str) -> Result<Version> {
    let cleaned = version_str.trim_start_matches('v');
    Version::parse(cleaned).map_err(|e| anyhow!("Invalid version '{}': {}", version_str, e))
}

/// Compare versions, returns true if remote is newer than current
pub fn is_newer(remote: &str, current: &str) -> Result<bool> {
    let remote_ver = parse_version(remote)?;
    let current_ver = parse_version(current)?;
    Ok(remote_ver > current_ver)
}

/// Get current binary version
pub fn current() -> &'static str {
    CURRENT_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert!(parse_version("1.0.0").is_ok());
        assert!(parse_version("v1.0.0").is_ok());
        assert!(parse_version("0.1.0").is_ok());
        assert!(parse_version("invalid").is_err());
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("1.0.0", "0.1.0").unwrap());
        assert!(is_newer("v1.0.0", "0.9.9").unwrap());
        assert!(!is_newer("0.1.0", "1.0.0").unwrap());
        assert!(!is_newer("1.0.0", "1.0.0").unwrap());
    }
}
