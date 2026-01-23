use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::io::Write;
use std::path::Path;

const GITHUB_API_URL: &str = "https://api.github.com/repos/K-NRS/repo-cli/releases/latest";
const USER_AGENT: &str = concat!("repo-cli/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub assets: Vec<Asset>,
    pub html_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Fetch the latest release info from GitHub
pub fn fetch_latest_release() -> Result<Release> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    let response = client
        .get(GITHUB_API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("Failed to fetch releases from GitHub")?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(anyhow!("No releases found for repo-cli"));
    }

    if !response.status().is_success() {
        return Err(anyhow!(
            "GitHub API error: {} {}",
            response.status(),
            response.text().unwrap_or_default()
        ));
    }

    response
        .json::<Release>()
        .context("Failed to parse release JSON")
}

/// Get the appropriate asset for the current platform
pub fn get_platform_asset(release: &Release) -> Result<&Asset> {
    let target = get_target_triple();
    let extension = if cfg!(windows) { ".zip" } else { ".tar.gz" };
    let expected_name = format!("repo-{}{}", target, extension);

    release
        .assets
        .iter()
        .find(|a| a.name == expected_name)
        .ok_or_else(|| anyhow!("No asset found for platform: {}", target))
}

/// Download asset to destination path with progress bar
pub fn download_asset(asset: &Asset, dest: &Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    let mut response = client
        .get(&asset.browser_download_url)
        .send()
        .context("Failed to download asset")?;

    if !response.status().is_success() {
        return Err(anyhow!("Download failed: {}", response.status()));
    }

    let total_size = asset.size;
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut file = std::fs::File::create(dest).context("Failed to create download file")?;
    let mut downloaded: u64 = 0;
    let mut buffer = [0u8; 8192];

    loop {
        use std::io::Read;
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;
        pb.set_position(downloaded);
    }

    pb.finish_with_message("Download complete");
    Ok(())
}

/// Get the target triple for the current platform
fn get_target_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-gnu";

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "x86_64-pc-windows-msvc";

    #[cfg(not(any(
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    return "unknown";
}
