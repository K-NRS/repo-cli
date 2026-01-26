pub mod tui;

use std::io::{self, Write};

use anyhow::{bail, Context, Result};
use colored::Colorize;
use git2::Repository;

use crate::ai::{detect_provider, generate_commit_message, AiProvider};
use crate::git::{
    create_commit, get_staged_diff, get_staged_files, get_unstaged_diff, get_unstaged_files,
    get_working_tree_status, has_staged_changes, stage_all,
};

use crate::config::Config;
use tui::{run_commit_tui, CommitApp, TuiResult};

/// Main entry point for the commit workflow
pub fn run_commit_workflow(
    repo: Repository,
    cli_ai: Option<String>,
    interactive: bool,
) -> Result<()> {
    // Check for staged changes
    if !has_staged_changes(&repo)? {
        let status = get_working_tree_status(&repo)?;
        let unstaged = status.modified + status.untracked;

        if unstaged == 0 {
            bail!("Nothing to commit. Working tree clean.");
        }

        // Non-interactive: auto-stage all
        if !interactive {
            stage_all(&repo)?;
            println!("{} Staged {} file(s)", "✓".green(), unstaged);
        } else {
            // Interactive prompt loop
            loop {
            print!(
                "{} {} unstaged file(s). Stage all? [Y/n] {}  ",
                "?".yellow().bold(),
                unstaged,
                "l=list d=diff".dimmed()
            );
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
                    for (path, status) in &files {
                        let marker = match status {
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
                    bail!("Cancelled.");
                }
                _ => {
                    println!("  {}", "y=stage, n=cancel, l=list, d=diff".dimmed());
                }
            }
        }
        }
    }

    // Load config
    let config = Config::load().unwrap_or_default();

    // Resolve AI provider: CLI flag > config > auto-detect
    let provider = resolve_provider(cli_ai, &config)?;

    // Get staged diff and files
    let diff = get_staged_diff(&repo)?;
    let staged_files = get_staged_files(&repo)?;

    println!(
        "{} Generating commit message with {}...",
        "●".cyan(),
        provider.name().bold()
    );

    // Generate initial message with configured style
    let style = config.commit_style.as_deref();
    let mut message = generate_commit_message(provider, &diff, style)?;

    if !interactive {
        // Non-interactive: commit directly
        let oid = create_commit(&repo, &message)?;
        println!("{}", message.dimmed());
        println!(
            "{} Committed: {}",
            "✓".green().bold(),
            &oid.to_string()[..7]
        );
        return Ok(());
    }

    // Interactive: show message and prompt
    loop {
        println!();
        println!("{}", "─".repeat(50).dimmed());
        for line in message.lines() {
            println!("  {}", line);
        }
        println!("{}", "─".repeat(50).dimmed());
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
                let oid = create_commit(&repo, &message)?;
                println!(
                    "{} Committed: {}",
                    "✓".green().bold(),
                    &oid.to_string()[..7]
                );
                break;
            }
            "e" => {
                // Open TUI for editing
                let app = CommitApp::new(message.clone(), diff.clone(), provider, staged_files.clone());
                let (final_message, result) = run_commit_tui(app)?;

                match result {
                    TuiResult::Commit => {
                        let oid = create_commit(&repo, &final_message)?;
                        println!(
                            "{} Committed: {}",
                            "✓".green().bold(),
                            &oid.to_string()[..7]
                        );
                        break;
                    }
                    TuiResult::Cancel => {
                        // Return to prompt with current message
                        message = final_message;
                    }
                }
            }
            "r" => {
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
