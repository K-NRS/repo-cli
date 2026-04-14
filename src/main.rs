use anyhow::Result;
use clap::{Parser, Subcommand};

use repo_cli::config::Config;
use repo_cli::git::{fetch_all_remotes, gather_summary, open_repo, print_fetch_warnings};
use repo_cli::render::render_static;
use repo_cli::terminal::{restore_title, set_title, repo_display_name};

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

        /// AI model override (passed to the provider CLI)
        #[arg(short, long)]
        model: Option<String>,

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

        /// AI model override (passed to the provider CLI)
        #[arg(short, long)]
        model: Option<String>,

        /// Amend the last commit instead of creating a new one
        #[arg(long)]
        amend: bool,
    },

    /// Interactive commit (alias for `commit`)
    Ic {
        /// AI provider to use (claude, codex, gemini)
        #[arg(long)]
        ai: Option<String>,

        /// AI model override (passed to the provider CLI)
        #[arg(short, long)]
        model: Option<String>,

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
        /// Version to release (e.g., 0.1.0 or v0.1.0). Omit with --auto.
        version: Option<String>,

        /// Create as draft release
        #[arg(long)]
        draft: bool,

        /// Print the plan without creating the release
        #[arg(long)]
        dry_run: bool,

        /// Read release notes from file (skips gh --generate-notes)
        #[arg(long, value_name = "PATH")]
        notes_from_file: Option<String>,

        /// Compute next version from conventional commits since last tag
        #[arg(long)]
        auto: bool,
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

    /// Explore repository history and branches interactively
    Explore {
        /// Start on a specific tab: history, branches
        #[arg(value_name = "TAB")]
        tab: Option<String>,

        /// Number of commits to load per page
        #[arg(long, default_value = "50")]
        page_size: usize,
    },

    /// Quick explore (alias for `explore`)
    E {
        /// Start on a specific tab: history, branches
        #[arg(value_name = "TAB")]
        tab: Option<String>,

        /// Number of commits to load per page
        #[arg(long, default_value = "50")]
        page_size: usize,
    },

    /// Show a multi-repo feed for a directory or saved group
    Feed {
        /// Path to scan, or alias of a saved group (omit to open group picker)
        #[arg(value_name = "TARGET")]
        target: Option<String>,

        /// Filter expression, e.g. "status:dirty author:keren text:refactor"
        #[arg(short, long)]
        filter: Option<String>,

        /// Force interactive TUI mode
        #[arg(short, long)]
        interactive: bool,

        /// Max scan depth (ad-hoc paths only)
        #[arg(long, default_value = "3")]
        depth: usize,
    },

    /// Manage named groups of repositories
    Groups {
        #[command(subcommand)]
        action: Option<GroupsAction>,
    },
}

#[derive(Subcommand, Debug)]
enum GroupsAction {
    /// List all saved groups
    List,
    /// Create a new group via interactive picker
    New {
        /// Initial scan root (prefilled in picker)
        #[arg(long)]
        root: Option<String>,
    },
    /// Edit an existing group via picker
    Edit {
        /// Group alias
        alias: String,
    },
    /// Remove a group
    Rm {
        /// Group alias
        alias: String,
    },
    /// Show group configuration
    Show {
        /// Group alias
        alias: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set terminal title based on command
    let subtitle = match &cli.command {
        Some(Command::Commit { .. }) | Some(Command::C { .. }) | Some(Command::Ic { .. }) => "commit",
        Some(Command::Update { .. }) => "update",
        Some(Command::Release { .. }) => "release",
        Some(Command::Stars) => "stars",
        Some(Command::Forks) => "forks",
        Some(Command::Sync { .. }) | Some(Command::S { .. }) => "sync",
        Some(Command::Reword { .. }) => "reword",
        Some(Command::Craft { .. }) => "craft",
        Some(Command::Explore { .. }) | Some(Command::E { .. }) => "explore",
        Some(Command::Feed { .. }) => "feed",
        Some(Command::Groups { .. }) => "groups",
        None => "",
    };

    // Try to get repo name for title
    let repo_name = cli.path.as_deref()
        .and_then(|p| open_repo(Some(std::path::Path::new(p))).ok())
        .or_else(|| open_repo(None).ok())
        .map(|r| repo_display_name(&r));

    let title = match (&repo_name, subtitle) {
        (Some(name), "") => format!("repo · {}", name),
        (Some(name), sub) => format!("repo {} · {}", sub, name),
        (None, "") => "repo".to_string(),
        (None, sub) => format!("repo {}", sub),
    };
    set_title(&title);

    let result = match cli.command {
        Some(Command::Commit { ai, model, no_interactive, amend }) => {
            run_commit_command(ai, model, no_interactive, amend, cli.path)
        }
        Some(Command::C { ai, model, amend }) => run_commit_command(ai, model, true, amend, cli.path),
        Some(Command::Ic { ai, model, amend }) => run_commit_command(ai, model, false, amend, cli.path),
        Some(Command::Update { check }) => run_update_command(check),
        Some(Command::Release { version, draft, dry_run, notes_from_file, auto }) => {
            run_release_command(version, draft, dry_run, notes_from_file, auto)
        }
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
        Some(Command::Explore { tab, page_size })
        | Some(Command::E { tab, page_size }) => {
            run_explore_command(tab, page_size, cli.path)
        }
        Some(Command::Feed { target, filter, interactive, depth }) => {
            run_feed_command(target, filter, interactive || cli.interactive, depth, cli.no_color)
        }
        Some(Command::Groups { action }) => run_groups_command(action),
        None => run_summary_command(&cli),
    };

    restore_title();
    result
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
        use repo_cli::explore;
        explore::run_explore(repo, summary, Some("summary".to_string()), 50, &config)?;
    } else {
        render_static(&summary, cli.graph, !cli.no_color, cli.stashes);
    }

    Ok(())
}

fn run_commit_command(ai: Option<String>, model: Option<String>, no_interactive: bool, amend: bool, path: Option<String>) -> Result<()> {
    use repo_cli::commit::run_commit_workflow;

    let repo = match &path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    run_commit_workflow(repo, ai, model, !no_interactive, amend)
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

fn run_release_command(
    version: Option<String>,
    draft: bool,
    dry_run: bool,
    notes_from_file: Option<String>,
    auto: bool,
) -> Result<()> {
    repo_cli::release::create_release(repo_cli::release::ReleaseOpts {
        version,
        draft,
        dry_run,
        notes_from_file,
        auto,
    })
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

fn run_explore_command(tab: Option<String>, page_size: usize, path: Option<String>) -> Result<()> {
    use repo_cli::explore;

    let mut repo = match &path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    let config = Config::load().unwrap_or_default();
    let summary = gather_summary(&mut repo, 5)?;

    explore::run_explore(repo, summary, tab, page_size, &config)
}

fn run_feed_command(
    target: Option<String>,
    filter: Option<String>,
    interactive: bool,
    depth: usize,
    no_color: bool,
) -> Result<()> {
    use colored::Colorize;
    use repo_cli::workspace::groups::{expand_path, GroupsFile};
    use repo_cli::workspace::render::render_workspace;
    use repo_cli::workspace::scan::{scan_group, scan_path, ScanOptions};
    use repo_cli::workspace::tui::picker::create_group_interactive;
    use repo_cli::workspace::tui::run_workspace_tui;
    use repo_cli::workspace::filter::parse_filters;

    let config = Config::load().unwrap_or_default();

    // Resolve target: path, alias, or picker
    let summary = match target.as_deref() {
        None => {
            // No target → open picker to create/pick a group
            println!("{} no target given — opening group picker", "→".cyan());
            let Some(alias) = create_group_interactive(None)? else {
                println!("{} cancelled", "·".dimmed());
                return Ok(());
            };
            let file = GroupsFile::load()?;
            let group = file
                .find(&alias)
                .ok_or_else(|| anyhow::anyhow!("group '{}' not found after save", alias))?;
            scan_group(group, &config)?
        }
        Some(s) if looks_like_path(s) => {
            let path = expand_path(s);
            if !path.exists() {
                anyhow::bail!("path does not exist: {}", path.display());
            }
            let opts = ScanOptions {
                max_depth: depth,
                ..Default::default()
            };
            scan_path(&path, &opts, &config)?
        }
        Some(alias) => {
            let file = GroupsFile::load()?;
            let group = file.find(alias).ok_or_else(|| {
                anyhow::anyhow!("unknown alias '{}' (and not a path). Run `repo groups list`", alias)
            })?;
            scan_group(group, &config)?
        }
    };

    let filter_str = filter.unwrap_or_default();

    if interactive {
        run_workspace_tui(summary, filter_str)?;
    } else {
        let filters = parse_filters(&filter_str);
        render_workspace(&summary, &filters, !no_color);
    }

    Ok(())
}

fn looks_like_path(s: &str) -> bool {
    s.starts_with('/')
        || s.starts_with('~')
        || s.starts_with("./")
        || s.starts_with("../")
        || s == "."
        || s == ".."
        || std::path::Path::new(s).exists()
}

fn run_groups_command(action: Option<GroupsAction>) -> Result<()> {
    use colored::Colorize;
    use repo_cli::workspace::groups::GroupsFile;
    use repo_cli::workspace::tui::picker::{create_group_interactive, edit_group_interactive};

    let action = action.unwrap_or(GroupsAction::List);
    match action {
        GroupsAction::List => {
            let file = GroupsFile::load()?;
            if file.groups.is_empty() {
                println!("{} no groups yet — run `repo groups new`", "·".dimmed());
                return Ok(());
            }
            println!("{} ({})", "GROUPS".bold(), file.groups.len());
            for g in &file.groups {
                let root = g.scan_root.as_deref().unwrap_or("(pinned only)");
                let extras = if !g.pinned.is_empty() {
                    format!(" +{} pinned", g.pinned.len())
                } else {
                    String::new()
                };
                let unpinned = if !g.unpinned.is_empty() {
                    format!(" -{} excluded", g.unpinned.len())
                } else {
                    String::new()
                };
                println!(
                    "   {} {}  {}{}{}",
                    "▣".cyan(),
                    g.alias.bold(),
                    root.dimmed(),
                    extras.dimmed(),
                    unpinned.dimmed(),
                );
            }
        }
        GroupsAction::New { root } => {
            match create_group_interactive(root)? {
                Some(alias) => println!("{} created group {}", "✓".green(), alias.bold()),
                None => println!("{} cancelled", "·".dimmed()),
            }
        }
        GroupsAction::Edit { alias } => {
            if edit_group_interactive(&alias)? {
                println!("{} updated group {}", "✓".green(), alias.bold());
            } else {
                println!("{} group '{}' not found or cancelled", "!".yellow(), alias);
            }
        }
        GroupsAction::Rm { alias } => {
            let mut file = GroupsFile::load()?;
            if file.remove(&alias) {
                file.save()?;
                println!("{} removed group {}", "✓".green(), alias.bold());
            } else {
                println!("{} no group named '{}'", "!".yellow(), alias);
            }
        }
        GroupsAction::Show { alias } => {
            let file = GroupsFile::load()?;
            match file.find(&alias) {
                Some(g) => {
                    let toml_str = toml::to_string_pretty(g)
                        .unwrap_or_else(|_| "(serialization failed)".to_string());
                    println!("{}", toml_str);
                }
                None => println!("{} no group named '{}'", "!".yellow(), alias),
            }
        }
    }

    Ok(())
}
