use anyhow::Result;
use clap::{Parser, Subcommand};

use repo_cli::config::Config;
use repo_cli::git::{fetch_all_remotes, gather_summary, open_repo, print_fetch_warnings};
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

    /// Fetch from remotes before showing summary
    #[arg(long, global = true)]
    fetch: bool,

    /// Skip fetching even if auto_fetch is enabled in config
    #[arg(long, global = true)]
    no_fetch: bool,

    /// Show stash details (only count shown by default)
    #[arg(long, global = true)]
    stashes: bool,

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
        #[arg(short = 'y', long)]
        no_interactive: bool,

        /// Amend the last commit instead of creating a new one
        #[arg(long)]
        amend: bool,
    },

    /// Quick commit (non-interactive, alias for `commit --no-interactive`)
    C {
        /// AI provider to use (claude, codex, gemini)
        #[arg(long)]
        ai: Option<String>,

        /// Amend the last commit instead of creating a new one
        #[arg(long)]
        amend: bool,
    },

    /// Interactive commit (alias for `commit`)
    Ic {
        /// AI provider to use (claude, codex, gemini)
        #[arg(long)]
        ai: Option<String>,

        /// Amend the last commit instead of creating a new one
        #[arg(long)]
        amend: bool,
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

    /// Pull and push to sync with remote
    Sync {
        /// Use rebase instead of merge when pulling
        #[arg(long)]
        rebase: bool,
    },

    /// Quick sync (alias for `sync`)
    S {
        /// Use rebase instead of merge when pulling
        #[arg(long)]
        rebase: bool,
    },

    /// Reword past commit messages via interactive rebase
    Reword {
        /// Auto-select last N commits
        #[arg(long)]
        last: Option<usize>,

        /// Auto-select all displayed commits
        #[arg(long)]
        all: bool,

        /// Number of commits to display for selection (default: 20)
        #[arg(long, default_value = "20")]
        count: usize,

        /// Use $EDITOR instead of inline prompt
        #[arg(long)]
        editor: bool,
    },

    /// Surgical commit design — reword, split, squash, reorder, drop via TUI
    Craft {
        /// Number of commits to display (default: 20)
        #[arg(long, default_value = "20")]
        count: usize,

        /// Pre-select last N commits
        #[arg(long)]
        last: Option<usize>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Commit { ai, no_interactive, amend }) => {
            run_commit_command(ai, no_interactive, amend, cli.path)
        }
        Some(Command::C { ai, amend }) => run_commit_command(ai, true, amend, cli.path),
        Some(Command::Ic { ai, amend }) => run_commit_command(ai, false, amend, cli.path),
        Some(Command::Update { check }) => run_update_command(check),
        Some(Command::Release { version, draft }) => run_release_command(&version, draft),
        Some(Command::Stars) => run_stars_command(cli.path),
        Some(Command::Forks) => run_forks_command(cli.path),
        Some(Command::Sync { rebase }) => run_sync_command(rebase, cli.path),
        Some(Command::S { rebase }) => run_sync_command(rebase, cli.path),
        Some(Command::Reword { last, all, count, editor }) => {
            run_reword_command(last, all, count, editor, cli.path)
        }
        Some(Command::Craft { count, last }) => {
            run_craft_command(count, last, cli.path)
        }
        None => run_summary_command(&cli),
    }
}

fn run_summary_command(cli: &Cli) -> Result<()> {
    let mut repo = match &cli.path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    // Determine if we should fetch: CLI flags override config
    let config = Config::load().unwrap_or_default();
    let should_fetch = if cli.no_fetch {
        false
    } else if cli.fetch {
        true
    } else {
        config.auto_fetch
    };

    if should_fetch {
        let repo_path = repo.workdir().unwrap_or_else(|| repo.path());
        let warnings = fetch_all_remotes(repo_path);
        print_fetch_warnings(&warnings);
    }

    let summary = gather_summary(&mut repo, cli.commits)?;

    if cli.interactive {
        run_tui(summary)?;
    } else {
        render_static(&summary, cli.graph, !cli.no_color, cli.stashes);
    }

    Ok(())
}

fn run_commit_command(ai: Option<String>, no_interactive: bool, amend: bool, path: Option<String>) -> Result<()> {
    use repo_cli::commit::run_commit_workflow;

    let repo = match &path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    run_commit_workflow(repo, ai, !no_interactive, amend)
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

fn run_sync_command(rebase: bool, path: Option<String>) -> Result<()> {
    use colored::Colorize;
    use std::process::Command as Cmd;

    let repo_path = path.as_deref().unwrap_or(".");

    // Check for uncommitted changes
    let status = Cmd::new("git")
        .args(["-C", repo_path, "status", "--porcelain"])
        .output()?;

    if !status.stdout.is_empty() {
        eprintln!(
            "{} uncommitted changes, stash or commit first",
            "⚠".yellow()
        );
        return Ok(());
    }

    // Pull
    print!("{} pulling...", "↓".cyan());
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut pull_args = vec!["-C", repo_path, "pull"];
    if rebase {
        pull_args.push("--rebase");
    }

    let pull = Cmd::new("git").args(&pull_args).output()?;

    if !pull.status.success() {
        println!(" {}", "failed".red());
        let stderr = String::from_utf8_lossy(&pull.stderr);
        if !stderr.is_empty() {
            eprintln!("{}", stderr);
        }
        return Ok(());
    }
    println!(" {}", "ok".green());

    // Push
    print!("{} pushing...", "↑".cyan());
    std::io::Write::flush(&mut std::io::stdout())?;

    let push = Cmd::new("git")
        .args(["-C", repo_path, "push"])
        .output()?;

    if !push.status.success() {
        println!(" {}", "failed".red());
        let stderr = String::from_utf8_lossy(&push.stderr);
        if !stderr.is_empty() {
            eprintln!("{}", stderr);
        }
        return Ok(());
    }
    println!(" {}", "ok".green());

    println!("{} synced", "✓".green());
    Ok(())
}

fn run_reword_command(
    last: Option<usize>,
    all: bool,
    count: usize,
    editor: bool,
    path: Option<String>,
) -> Result<()> {
    use repo_cli::reword::{run_reword, RewordArgs};

    let repo = match &path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    run_reword(&repo, RewordArgs { last, all, count, editor })
}

fn run_craft_command(count: usize, last: Option<usize>, path: Option<String>) -> Result<()> {
    use repo_cli::craft::{run_craft, CraftArgs};

    let repo = match &path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    run_craft(&repo, CraftArgs { count, last })
}
