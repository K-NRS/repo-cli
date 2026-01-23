use anyhow::Result;
use clap::{Parser, Subcommand};

use repo_cli::git::{gather_summary, open_repo};
use repo_cli::render::{render_static, run_tui};

#[derive(Parser, Debug)]
#[command(name = "repo")]
#[command(about = "A visual git repository summary tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Run in interactive TUI mode
    #[arg(short, long, global = true)]
    interactive: bool,

    /// Show full ASCII branch graph
    #[arg(long, global = true)]
    graph: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Number of recent commits to show (default: 5)
    #[arg(short = 'n', long, default_value = "5", global = true)]
    commits: usize,

    /// Path to git repository (defaults to current directory)
    #[arg(value_name = "PATH", global = true)]
    path: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate AI-powered commit message for staged changes
    Commit {
        /// AI provider to use (claude, codex, gemini)
        #[arg(long)]
        ai: Option<String>,

        /// Commit directly without interactive TUI
        #[arg(long)]
        no_interactive: bool,
    },

    /// Check for updates and optionally self-update
    Update {
        /// Only check for updates without installing
        #[arg(long)]
        check: bool,
    },

    /// Create a GitHub release (requires gh CLI)
    Release {
        /// Version to release (e.g., 0.1.0 or v0.1.0)
        version: String,

        /// Create as draft release
        #[arg(long)]
        draft: bool,
    },

    /// List users who starred this repository
    Stars,

    /// List forks of this repository
    Forks,

    /// Initialize auto-release workflow for a project
    Init {
        /// Target project path (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,

        /// Project type (auto-detected if not specified)
        /// Options: rust, bun, pnpm, nextjs, nodejs, react-native, xcode, go, python, generic
        #[arg(long)]
        lang: Option<String>,

        /// Overwrite existing workflow without prompting
        #[arg(long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Commit { ai, no_interactive }) => {
            run_commit_command(ai, no_interactive, cli.path)
        }
        Some(Command::Update { check }) => run_update_command(check),
        Some(Command::Release { version, draft }) => run_release_command(&version, draft),
        Some(Command::Stars) => run_stars_command(cli.path),
        Some(Command::Forks) => run_forks_command(cli.path),
        Some(Command::Init { path, lang, force }) => run_init_command(&path, lang, force),
        None => run_summary_command(&cli),
    }
}

fn run_summary_command(cli: &Cli) -> Result<()> {
    let mut repo = match &cli.path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    let summary = gather_summary(&mut repo, cli.commits)?;

    if cli.interactive {
        run_tui(summary)?;
    } else {
        render_static(&summary, cli.graph, !cli.no_color);
    }

    Ok(())
}

fn run_commit_command(ai: Option<String>, no_interactive: bool, path: Option<String>) -> Result<()> {
    use repo_cli::commit::run_commit_workflow;

    let repo = match &path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    run_commit_workflow(repo, ai, !no_interactive)
}

fn run_update_command(check_only: bool) -> Result<()> {
    use colored::Colorize;
    use repo_cli::update;

    if check_only {
        match update::check_for_update()? {
            Some(release) => {
                println!(
                    "{} New version available: {} → {}",
                    "↑".yellow(),
                    update::CURRENT_VERSION.dimmed(),
                    release.tag_name.green()
                );
                println!("  Run `repo update` to install");
            }
            None => {
                println!(
                    "{} You're running the latest version ({})",
                    "✓".green(),
                    update::CURRENT_VERSION
                );
            }
        }
        Ok(())
    } else {
        update::perform_update()
    }
}

fn run_release_command(version: &str, draft: bool) -> Result<()> {
    repo_cli::release::create_release(version, draft)
}

fn run_stars_command(path: Option<String>) -> Result<()> {
    use colored::Colorize;
    use repo_cli::git::{get_stargazers, open_repo};

    let repo = match &path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    let stargazers = get_stargazers(&repo)?;

    if stargazers.is_empty() {
        println!("{}", "No stargazers yet".dimmed());
        return Ok(());
    }

    println!("{} ({})", "STARGAZERS".bold(), stargazers.len());
    for user in &stargazers {
        println!("   {} {}", "★".yellow(), user.login);
    }

    if stargazers.len() == 100 {
        println!("   {}", "... showing first 100".dimmed());
    }

    Ok(())
}

fn run_init_command(path: &str, lang: Option<String>, force: bool) -> Result<()> {
    repo_cli::run_init(path, lang, force)
}

fn run_forks_command(path: Option<String>) -> Result<()> {
    use colored::Colorize;
    use repo_cli::git::{get_forks, open_repo};

    let repo = match &path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    let forks = get_forks(&repo)?;

    if forks.is_empty() {
        println!("{}", "No forks yet".dimmed());
        return Ok(());
    }

    println!("{} ({})", "FORKS".bold(), forks.len());
    for fork in &forks {
        let stars_str = if fork.stars > 0 {
            format!(" ★{}", fork.stars).yellow().to_string()
        } else {
            String::new()
        };
        println!("   {} {}{}", "⑂".dimmed(), fork.repo_name, stars_str);
    }

    if forks.len() == 100 {
        println!("   {}", "... showing first 100".dimmed());
    }

    Ok(())
}
