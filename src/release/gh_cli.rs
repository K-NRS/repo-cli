use anyhow::{anyhow, Context, Result};
use std::process::Command;

/// Check if gh CLI is installed and authenticated
pub fn check_gh_cli() -> Result<()> {
    let output = Command::new("gh")
        .arg("--version")
        .output()
        .context("gh CLI not found. Install from https://cli.github.com")?;

    if !output.status.success() {
        return Err(anyhow!("gh CLI not working properly"));
    }

    // Check authentication
    let auth_output = Command::new("gh")
        .args(["auth", "status"])
        .output()
        .context("Failed to check gh auth status")?;

    if !auth_output.status.success() {
        return Err(anyhow!(
            "gh CLI not authenticated. Run: gh auth login"
        ));
    }

    Ok(())
}

/// Create a GitHub release using gh CLI
pub fn create_release(
    version: &str,
    title: Option<&str>,
    notes: Option<&str>,
    draft: bool,
) -> Result<String> {
    check_gh_cli()?;

    let tag = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{}", version)
    };

    let mut args = vec!["release", "create", &tag];

    let title_str: String;
    if let Some(t) = title {
        title_str = t.to_string();
        args.push("--title");
        args.push(&title_str);
    } else {
        title_str = format!("Release {}", tag);
        args.push("--title");
        args.push(&title_str);
    }

    let notes_str: String;
    if let Some(n) = notes {
        notes_str = n.to_string();
        args.push("--notes");
        args.push(&notes_str);
    } else {
        args.push("--generate-notes");
    }

    if draft {
        args.push("--draft");
    }

    let output = Command::new("gh")
        .args(&args)
        .output()
        .context("Failed to execute gh release create")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to create release: {}", stderr));
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(url)
}

/// Get the remote repository URL
pub fn get_repo_url() -> Result<String> {
    let output = Command::new("gh")
        .args(["repo", "view", "--json", "url", "-q", ".url"])
        .output()
        .context("Failed to get repo URL")?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get repository URL"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
