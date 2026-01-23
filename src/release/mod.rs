pub mod gh_cli;

use anyhow::Result;
use colored::Colorize;

/// Create a new GitHub release
pub fn create_release(version: &str, draft: bool) -> Result<()> {
    println!("{}", "Checking gh CLI...".cyan());
    gh_cli::check_gh_cli()?;

    let tag = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{}", version)
    };

    println!(
        "Creating {} release {}...",
        if draft { "draft" } else { "public" },
        tag.green()
    );

    let url = gh_cli::create_release(version, None, None, draft)?;

    println!("\n{} Release created!", "✓".green());
    println!("  {}", url.cyan());

    if draft {
        println!(
            "\n{} This is a draft release. Publish it from GitHub when ready.",
            "→".yellow()
        );
    } else {
        println!(
            "\n{} GitHub Actions will now build and attach binaries.",
            "→".yellow()
        );
    }

    Ok(())
}
