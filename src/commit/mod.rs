pub mod tui;

use std::io::{self, Write};

use anyhow::{bail, Context, Result};
use colored::Colorize;
use git2::Repository;

use crate::ai::{detect_provider, generate_commit_message, AiProvider};
use crate::git::{
    amend_commit, create_commit, get_amend_diff, get_last_commit_message, get_staged_diff,
    get_staged_files, get_unstaged_diff, get_unstaged_files, get_working_tree_status,
    has_staged_changes, stage_all,
};

use crate::config::{Config, MessageBoxStyle};
use crate::update;
use tui::{run_commit_tui, CommitApp, TuiResult};

/// Silently check for updates and print hint if available
fn notify_update_available() {
    // Don't fail commit if update check fails
    if let Ok(Some(release)) = update::check_for_update() {
        println!(
            "\n{} Update available: {} → {} (run: repo update)",
            "↑".yellow(),
            update::CURRENT_VERSION.dimmed(),
            release.tag_name.green()
        );
    }
}

fn print_message_box(message: &str, style: MessageBoxStyle) {
    let width = 50;
    let lines: Vec<&str> = message.lines().collect();

    match style {
        MessageBoxStyle::Box => {
            println!("{}", format!("╭{}╮", "─".repeat(width)).dimmed());
            for line in &lines {
                let content = format!("  {}", line);
                let pad = width.saturating_sub(content.len());
                println!(
                    "{}{}{}{}",
                    "│".dimmed(),
                    content,
                    " ".repeat(pad),
                    "│".dimmed()
                );
            }
            println!("{}", format!("╰{}╯", "─".repeat(width)).dimmed());
        }
        MessageBoxStyle::DoubleLine => {
            println!("{}", "═".repeat(width + 2).dimmed());
            for line in &lines {
                println!("  {}", line);
            }
            println!("{}", "═".repeat(width + 2).dimmed());
        }
        MessageBoxStyle::TitleBox => {
            let title = " Commit Message ";
            let side = (width.saturating_sub(title.len())) / 2;
            println!(
                "{}{}{}",
                "─".repeat(side).dimmed(),
                title.bold(),
                "─".repeat(width - side - title.len()).dimmed()
            );
            for line in &lines {
                println!("  {}", line);
            }
            println!("{}", "─".repeat(width).dimmed());
        }
        MessageBoxStyle::Gutter => {
            for line in &lines {
                println!("  {} {}", "│".cyan(), line);
            }
        }
    }
}

/// Main entry point for the commit workflow
pub fn run_commit_workflow(
    repo: Repository,
    cli_ai: Option<String>,
    interactive: bool,
    amend: bool,
) -> Result<()> {
    let has_staged = has_staged_changes(&repo)?;
    let status = get_working_tree_status(&repo)?;
    let unstaged = status.modified + status.untracked;

    // Check if we have anything to work with
    if !has_staged && unstaged == 0 {
        if amend {
            // Amend with no changes: just edit message (handled below)
        } else {
            bail!("Nothing to commit. Working tree clean.");
        }
    }

    // Offer to stage unstaged files if nothing staged yet
    if !has_staged && unstaged > 0 {
        let prompt_msg = if amend {
            format!(
                "{} {} unstaged file(s). Add to last commit? [Y/n] {}  ",
                "?".yellow().bold(),
                unstaged,
                "l=list d=diff".dimmed()
            )
        } else {
            format!(
                "{} {} unstaged file(s). Stage all? [Y/n] {}  ",
                "?".yellow().bold(),
                unstaged,
                "l=list d=diff".dimmed()
            )
        };

        // Non-interactive: auto-stage all
        if !interactive {
            stage_all(&repo)?;
            println!("{} Staged {} file(s)", "✓".green(), unstaged);
        } else {
            // Interactive prompt loop
            loop {
                print!("{}", prompt_msg);
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                match input.trim().to_lowercase().as_str() {
                    "" | "y" => {
                        stage_all(&repo)?;
                        println!("{} Staged all changes", "✓".green());
                        break;
                    }
                    "l" => {
                        // List files
                        let files = get_unstaged_files(&repo)?;
                        println!();
                        for (path, file_status) in &files {
                            let marker = match file_status {
                                '?' => "?".yellow(),
                                'M' => "M".cyan(),
                                'D' => "D".red(),
                                'R' => "R".blue(),
                                _ => " ".normal(),
                            };
                            println!("  {} {}", marker, path);
                        }
                        println!();
                    }
                    "d" => {
                        // Show diff
                        let diff = get_unstaged_diff(&repo)?;
                        println!();
                        for line in diff.lines() {
                            if line.starts_with('+') && !line.starts_with("+++") {
                                println!("{}", line.green());
                            } else if line.starts_with('-') && !line.starts_with("---") {
                                println!("{}", line.red());
                            } else if line.starts_with("@@") {
                                println!("{}", line.cyan());
                            } else {
                                println!("{}", line);
                            }
                        }
                        println!();
                    }
                    "n" => {
                        if amend {
                            // For amend, 'n' means proceed without staging (edit message only)
                            break;
                        } else {
                            bail!("Cancelled.");
                        }
                    }
                    _ => {
                        if amend {
                            println!("  {}", "y=stage, n=skip (edit msg only), l=list, d=diff".dimmed());
                        } else {
                            println!("  {}", "y=stage, n=cancel, l=list, d=diff".dimmed());
                        }
                    }
                }
            }
        }
    }

    // For amend mode indicator
    let commit_fn: Box<dyn Fn(&Repository, &str) -> Result<git2::Oid>> = if amend {
        println!("{} Amending last commit", "●".yellow());
        Box::new(|r, m| amend_commit(r, m))
    } else {
        Box::new(|r, m| create_commit(r, m))
    };

    // Load config
    let config = Config::load().unwrap_or_default();

    // Resolve AI provider: CLI flag > config > auto-detect
    let provider = resolve_provider(cli_ai, &config)?;

    // Get diff: for amend use full diff (parent → index), else just staged
    let diff = if amend {
        get_amend_diff(&repo)?
    } else {
        get_staged_diff(&repo)?
    };
    let staged_files = get_staged_files(&repo)?;

    // For amend: keep existing message (squash-like behavior)
    // For new commit: generate with AI
    let mut message = if amend {
        let existing = get_last_commit_message(&repo)?;
        println!(
            "{} Keeping existing message {}",
            "●".cyan(),
            "(r=regenerate)".dimmed()
        );
        existing
    } else {
        println!(
            "{} Generating commit message with {}...",
            "●".cyan(),
            provider.name().bold()
        );
        let style = config.commit_style.as_deref();
        generate_commit_message(provider, &diff, style)?
    };

    let action_word = if amend { "Amended" } else { "Committed" };

    if !interactive {
        // Non-interactive: commit directly
        let oid = commit_fn(&repo, &message)?;
        println!("{}", message.bold());
        println!(
            "{} {}: {}",
            "✓".green(),
            action_word,
            &oid.to_string()[..7].dimmed()
        );
        notify_update_available();
        return Ok(());
    }

    // Interactive: show message and prompt
    loop {
        println!();
        print_message_box(&message, config.message_box_style);
        println!();

        print!(
            "{} Commit? [y/N] {}  ",
            "?".yellow().bold(),
            "e=edit r=regen d=diff".dimmed()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "y" => {
                let oid = commit_fn(&repo, &message)?;
                println!(
                    "{} {}: {}",
                    "✓".green(),
                    action_word,
                    &oid.to_string()[..7].dimmed()
                );
                notify_update_available();
                break;
            }
            "e" => {
                // Open TUI for editing
                let app = CommitApp::new(message.clone(), diff.clone(), provider, staged_files.clone());
                let (final_message, result) = run_commit_tui(app)?;

                match result {
                    TuiResult::Commit => {
                        let oid = commit_fn(&repo, &final_message)?;
                        println!(
                            "{} {}: {}",
                            "✓".green(),
                            action_word,
                            &oid.to_string()[..7].dimmed()
                        );
                        notify_update_available();
                        break;
                    }
                    TuiResult::Cancel => {
                        // Return to prompt with current message
                        message = final_message;
                    }
                }
            }
            "r" => {
                // Can't regenerate without a diff
                if diff.is_empty() {
                    println!("  {} No staged changes to regenerate from", "!".yellow());
                    continue;
                }

                // Regeneration sub-prompt
                print!(
                    "  {} Style: {} or custom instruction: ",
                    "↳".dimmed(),
                    "(c)oncise (l)onger (s)horter (d)etailed".dimmed()
                );
                io::stdout().flush()?;

                let mut style_input = String::new();
                io::stdin().read_line(&mut style_input)?;
                let style_input = style_input.trim();

                let style = match style_input.to_lowercase().as_str() {
                    "" | "c" => Some("Very concise, single line under 50 chars"),
                    "l" => Some("Longer with bullet points for details"),
                    "s" => Some("Shorter, minimal description"),
                    "d" => Some("Detailed with scope, body explaining why, and any breaking changes"),
                    _ => Some(style_input), // Custom instruction
                };

                println!("{} Regenerating...", "●".cyan());
                message = generate_commit_message(provider, &diff, style)?;
            }
            "d" => {
                if diff.is_empty() {
                    println!("  {} No staged changes", "!".yellow());
                    continue;
                }
                println!();
                for line in diff.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") {
                        println!("{}", line.green());
                    } else if line.starts_with('-') && !line.starts_with("---") {
                        println!("{}", line.red());
                    } else if line.starts_with("@@") {
                        println!("{}", line.cyan());
                    } else {
                        println!("{}", line);
                    }
                }
            }
            _ => {
                println!("{} Commit cancelled", "✗".red());
                break;
            }
        }
    }

    Ok(())
}

fn resolve_provider(cli_ai: Option<String>, config: &Config) -> Result<AiProvider> {
    // Priority 1: CLI flag
    if let Some(ref name) = cli_ai {
        return AiProvider::from_str(name)
            .context(format!("Unknown AI provider: {}. Use claude, codex, or gemini.", name));
    }

    // Priority 2: Config file
    if let Some(ref name) = config.default_ai {
        if let Some(provider) = AiProvider::from_str(name) {
            return Ok(provider);
        }
    }

    // Priority 3: Auto-detect
    detect_provider().context(
        "No AI CLI found. Install claude, codex, or gemini CLI, or specify with --ai flag.",
    )
}
