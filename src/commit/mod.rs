pub mod tui;

use std::io::{self, Write};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use colored::Colorize;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{self, ClearType},
    ExecutableCommand,
};
use git2::Repository;

use crate::ai::{detect_provider, generate_commit_message, AiProvider};
use crate::git::{
    amend_commit, create_commit, get_amend_diff, get_last_commit_message, get_staged_diff,
    get_staged_files, get_unstaged_diff, get_unstaged_files, get_working_tree_status,
    has_staged_changes, stage_all, stage_files,
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
    let lines: Vec<&str> = message.lines().collect();
    let max_content = lines.iter().map(|l| l.len() + 4).max().unwrap_or(0);
    let width = max_content.max(50);

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
                "l=list d=diff s=select".dimmed()
            )
        } else {
            format!(
                "{} {} unstaged file(s). Stage all? [Y/n] {}  ",
                "?".yellow().bold(),
                unstaged,
                "l=list d=diff s=select".dimmed()
            )
        };

        // Non-interactive: auto-stage all
        if !interactive {
            stage_all(&repo)?;
            println!("{} Staged {} file(s)", "✓".green(), unstaged);
        } else {
            let all_files = get_unstaged_files(&repo)?;

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
                        print_file_list(&all_files);
                    }
                    "d" => {
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
                    "s" => {
                        match run_file_selector(&all_files)? {
                            FileSelection::Selected(paths) if paths.is_empty() => {
                                println!("  {}", "No files selected.".dimmed());
                            }
                            FileSelection::Selected(paths) => {
                                let count = paths.len();
                                stage_files(&repo, &paths)?;
                                println!("{} Staged {} file(s):", "✓".green(), count);
                                for p in &paths {
                                    println!("  {}", p);
                                }
                                break;
                            }
                            FileSelection::Cancelled => {}
                        }
                    }
                    "n" => {
                        if amend {
                            break;
                        } else {
                            bail!("Cancelled.");
                        }
                    }
                    _ => {
                        if amend {
                            println!(
                                "  {}",
                                "y=stage all  n=skip  l=list  d=diff  s=select files".dimmed()
                            );
                        } else {
                            println!(
                                "  {}",
                                "y=stage all  n=cancel  l=list  d=diff  s=select files".dimmed()
                            );
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

/// Print numbered file list
fn print_file_list(files: &[(String, char)]) {
    println!();
    for (i, (path, file_status)) in files.iter().enumerate() {
        let marker = match file_status {
            '?' => "?".yellow(),
            'M' => "M".cyan(),
            'D' => "D".red(),
            'R' => "R".blue(),
            _ => " ".normal(),
        };
        let num = format!("{:>3}", i + 1).dimmed();
        println!("  {} {} {}", num, marker, path);
    }
    println!();
}

enum FileSelection {
    Selected(Vec<String>),
    Cancelled,
}

/// Interactive file selector with checkboxes
/// ↑/↓ navigate, Space toggle, a=all, n=none, Enter confirm, Esc cancel
fn run_file_selector(files: &[(String, char)]) -> Result<FileSelection> {
    use std::io::stdout;

    let mut selected = vec![true; files.len()]; // all selected by default
    let mut cursor_pos: usize = 0;

    // Enter raw mode for key-by-key input
    terminal::enable_raw_mode()?;

    let result = (|| -> Result<FileSelection> {
        let mut out = stdout();

        // Draw initial list
        draw_selector(&mut out, files, &selected, cursor_pos)?;

        loop {
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            if cursor_pos > 0 {
                                cursor_pos -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if cursor_pos + 1 < files.len() {
                                cursor_pos += 1;
                            }
                        }
                        KeyCode::Char(' ') => {
                            selected[cursor_pos] = !selected[cursor_pos];
                        }
                        KeyCode::Char('a') => {
                            selected.fill(true);
                        }
                        KeyCode::Char('i') => {
                            for s in selected.iter_mut() {
                                *s = !*s;
                            }
                        }
                        KeyCode::Char('n') => {
                            selected.fill(false);
                        }
                        KeyCode::Enter => {
                            // Clear the selector lines
                            clear_selector(&mut out, files.len())?;
                            let paths: Vec<String> = files
                                .iter()
                                .zip(selected.iter())
                                .filter(|(_, &sel)| sel)
                                .map(|((path, _), _)| path.clone())
                                .collect();
                            return Ok(FileSelection::Selected(paths));
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            clear_selector(&mut out, files.len())?;
                            return Ok(FileSelection::Cancelled);
                        }
                        _ => {}
                    }
                    // Redraw
                    redraw_selector(&mut out, files, &selected, cursor_pos)?;
                }
            }
        }
    })();

    terminal::disable_raw_mode()?;
    result
}

fn draw_selector(
    out: &mut impl Write,
    files: &[(String, char)],
    selected: &[bool],
    cursor_pos: usize,
) -> Result<()> {
    // Header
    write!(out, "\r\n")?;
    write!(
        out,
        "  {}\r\n",
        "↑↓=move  Space=toggle  a=all  n=none  i=invert  Enter=confirm  Esc=cancel"
            .to_string()
            .dimmed()
    )?;
    // File rows
    for (i, (path, file_status)) in files.iter().enumerate() {
        let pointer = if i == cursor_pos { ">" } else { " " };
        let checkbox = if selected[i] { "■" } else { "□" };
        let marker = match file_status {
            '?' => "?".yellow(),
            'M' => "M".cyan(),
            'D' => "D".red(),
            'R' => "R".blue(),
            _ => " ".normal(),
        };
        if i == cursor_pos {
            write!(
                out,
                "  {} {} {} {} \r\n",
                pointer.cyan().bold(),
                checkbox.green().bold(),
                marker,
                path.bold()
            )?;
        } else {
            write!(out, "  {} {} {} {}\r\n", pointer, checkbox.dimmed(), marker, path)?;
        }
    }
    // Selected count
    let count = selected.iter().filter(|&&s| s).count();
    write!(
        out,
        "\r\n  {} selected\r\n",
        format!("{}/{}", count, files.len()).cyan()
    )?;
    out.flush()?;
    Ok(())
}

fn redraw_selector(
    out: &mut impl Write,
    files: &[(String, char)],
    selected: &[bool],
    cursor_pos: usize,
) -> Result<()> {
    // Move cursor up: header(1) + files(n) + blank(1) + count(1) = n+3 lines
    let lines_up = files.len() + 3;
    out.execute(cursor::MoveUp(lines_up as u16))?;
    out.execute(terminal::Clear(ClearType::FromCursorDown))?;

    // Redraw from header
    write!(
        out,
        "  {}\r\n",
        "↑↓=move  Space=toggle  a=all  n=none  i=invert  Enter=confirm  Esc=cancel"
            .to_string()
            .dimmed()
    )?;
    for (i, (path, file_status)) in files.iter().enumerate() {
        let pointer = if i == cursor_pos { ">" } else { " " };
        let checkbox = if selected[i] { "■" } else { "□" };
        let marker = match file_status {
            '?' => "?".yellow(),
            'M' => "M".cyan(),
            'D' => "D".red(),
            'R' => "R".blue(),
            _ => " ".normal(),
        };
        if i == cursor_pos {
            write!(
                out,
                "  {} {} {} {} \r\n",
                pointer.cyan().bold(),
                checkbox.green().bold(),
                marker,
                path.bold()
            )?;
        } else {
            write!(out, "  {} {} {} {}\r\n", pointer, checkbox.dimmed(), marker, path)?;
        }
    }
    let count = selected.iter().filter(|&&s| s).count();
    write!(
        out,
        "\r\n  {} selected\r\n",
        format!("{}/{}", count, files.len()).cyan()
    )?;
    out.flush()?;
    Ok(())
}

fn clear_selector(out: &mut impl Write, file_count: usize) -> Result<()> {
    // Move up past: header(1) + newline(1) + files(n) + blank(1) + count(1) = n+4
    let lines_up = file_count + 4;
    out.execute(cursor::MoveUp(lines_up as u16))?;
    out.execute(terminal::Clear(ClearType::FromCursorDown))?;
    out.flush()?;
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
