pub mod github;
pub mod installer;
pub mod version;

use anyhow::{Context, Result};
use colored::Colorize;

pub use github::Release;
pub use version::CURRENT_VERSION;

/// Check for updates and return release info if available
pub fn check_for_update() -> Result<Option<Release>> {
    let release = match github::fetch_latest_release() {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("No releases found") {
                return Ok(None);
            }
            return Err(e);
        }
    };

    let is_newer = version::is_newer(&release.tag_name, version::current())?;
    if is_newer {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

/// Perform the update process
pub fn perform_update() -> Result<()> {
    println!("{}", "Checking for updates...".cyan());

    let release = match check_for_update()? {
        Some(r) => r,
        None => {
            println!(
                "{} You're running the latest version ({})",
                "✓".green(),
                version::current()
            );
            return Ok(());
        }
    };

    println!(
        "{} New version available: {} → {}",
        "↑".yellow(),
        version::current().dimmed(),
        release.tag_name.green()
    );

    if let Some(body) = &release.body {
        if !body.is_empty() {
            println!("\n{}", "Release notes:".cyan());
            println!("{}\n", body.dimmed());
        }
    }

    // Get platform-specific asset
    let asset = github::get_platform_asset(&release)?;
    println!("Downloading {}...", asset.name);

    // Download to temp dir
    let temp_dir = installer::get_temp_dir()?;
    let archive_path = temp_dir.join(&asset.name);

    github::download_asset(asset, &archive_path)?;

    // Extract archive
    println!("{}", "Extracting...".cyan());
    let extract_dir = temp_dir.join("extracted");
    let new_binary = installer::extract_archive(&archive_path, &extract_dir)?;

    // Replace current binary
    println!("{}", "Installing...".cyan());
    installer::replace_binary(&new_binary).context("Failed to install update")?;

    // Cleanup
    installer::cleanup_temp_dir()?;

    println!(
        "\n{} Updated to version {}",
        "✓".green(),
        release.tag_name.green()
    );

    Ok(())
}
