mod detect;
mod prompt;
mod templates;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

pub use detect::ProjectType;
use detect::detect_project_type;
use prompt::{prompt_overwrite, prompt_project_type};
use templates::get_workflow_template;

const WORKFLOW_PATH: &str = ".github/workflows/auto-release.yml";

/// Run the init command to scaffold auto-release workflow
pub fn run_init(path: &str, lang: Option<String>, force: bool) -> Result<()> {
    let project_path = Path::new(path).canonicalize().unwrap_or_else(|_| Path::new(path).to_path_buf());

    println!(
        "{} Initializing auto-release workflow in {}",
        "●".cyan(),
        project_path.display().to_string().dimmed()
    );

    // Determine project type
    let project_type = determine_project_type(&project_path, lang)?;

    println!(
        "{} Project type: {}",
        "✓".green(),
        project_type.name().bold()
    );

    // Check for existing workflow
    let workflow_file = project_path.join(WORKFLOW_PATH);
    if workflow_file.exists() && !force {
        if !prompt_overwrite()? {
            println!("{} Cancelled", "✗".red());
            return Ok(());
        }
    }

    // Create .github/workflows directory
    let workflows_dir = project_path.join(".github/workflows");
    fs::create_dir_all(&workflows_dir)
        .context("Failed to create .github/workflows directory")?;

    // Write workflow file
    let template = get_workflow_template(project_type);
    fs::write(&workflow_file, template)
        .context("Failed to write workflow file")?;

    println!(
        "{} Created {}",
        "✓".green(),
        WORKFLOW_PATH.cyan()
    );

    // Print next steps
    print_next_steps(project_type);

    Ok(())
}

fn determine_project_type(path: &Path, lang: Option<String>) -> Result<ProjectType> {
    // If explicit lang provided, use it
    if let Some(lang_str) = lang {
        return ProjectType::from_str(&lang_str)
            .ok_or_else(|| anyhow::anyhow!(
                "Unknown project type: {}. Valid options: rust, bun, pnpm, nextjs, nodejs, react-native, xcode, go, python, generic",
                lang_str
            ));
    }

    // Auto-detect
    let detected = detect_project_type(path);

    if let Some(pt) = detected {
        println!(
            "{} Detected: {} project",
            "✓".green(),
            pt.name().cyan()
        );
    } else {
        println!(
            "{} Could not auto-detect project type",
            "!".yellow()
        );
    }

    // Interactive selection
    prompt_project_type(detected)
}

fn print_next_steps(project_type: ProjectType) {
    println!();
    println!("{}", "Next steps:".bold());
    println!();
    println!("  1. Review the workflow at {}", WORKFLOW_PATH.cyan());
    println!("  2. Adjust branch name if not using 'main'");

    match project_type {
        ProjectType::Rust => {
            println!("  3. Update binary name in Package steps (currently 'app')");
        }
        ProjectType::Go => {
            println!("  3. Update binary name in Build step");
        }
        ProjectType::Bun | ProjectType::Pnpm | ProjectType::NodeJs | ProjectType::NextJs => {
            println!("  3. Ensure 'build' script exists in package.json");
        }
        ProjectType::ReactNative => {
            println!("  3. Configure EAS Build or native build tools separately");
        }
        ProjectType::Xcode => {
            println!("  3. Configure code signing for App Store builds");
        }
        ProjectType::Python => {
            println!("  3. Ensure version field exists in pyproject.toml or setup.py");
        }
        ProjectType::Generic => {
            println!("  3. Add custom build steps as needed");
        }
    }

    println!("  4. Commit with conventional commit messages:");
    println!("     - {} new feature (minor bump)", "feat:".green());
    println!("     - {} bug fix (patch bump)", "fix:".green());
    println!("     - {} breaking change (major bump)", "feat!:".green());
    println!();
}
