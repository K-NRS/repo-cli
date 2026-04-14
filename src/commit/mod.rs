pub mod tui;

use std::io::{self, Write};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use colored::Colorize;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use git2::Repository;

use crate::ai::{detect_provider, generate_commit_message, AiProvider};
use crate::git::{
    amend_commit, create_commit, get_amend_diff, get_last_commit_message, get_staged_diff,
    get_staged_files, get_unstaged_diff, get_unstaged_files, has_staged_changes, stage_all,
    stage_files,
};

use crate::config::{build_ignore_set, Config, MessageBoxStyle};
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
    cli_model: Option<String>,
    interactive: bool,
    amend: bool,
) -> Result<()> {
    let has_staged = has_staged_changes(&repo)?;

    // Load ignore patterns from config + .repoignore
    let config = Config::load().unwrap_or_default();
    let repo_root = repo
        .workdir()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let ignore_set = build_ignore_set(&config, &repo_root);

    // Filter unstaged files, separating ignored ones
    let all_unstaged = get_unstaged_files(&repo)?;
    let (visible_files, ignored_count) = if let Some(ref set) = ignore_set {
        let mut visible = Vec::new();
        let mut ignored = 0usize;
        for (path, status) in &all_unstaged {
            if set.is_match(path) {
                ignored += 1;
            } else {
                visible.push((path.clone(), *status));
            }
        }
        (visible, ignored)
    } else {
        (all_unstaged, 0)
    };

    let unstaged = visible_files.len();

    // Check if we have anything to work with
    if !has_staged && unstaged == 0 {
        if amend {
            // Amend with no changes: just edit message (handled below)
        } else {
            if ignored_count > 0 {
                println!(
                    "  {} {} file(s) hidden by .repoignore",
                    "⊘".dimmed(),
                    ignored_count
                );
            }
            bail!("Nothing to commit. Working tree clean.");
        }
    }

    // Offer to stage unstaged files if nothing staged yet
    if !has_staged && unstaged > 0 {
        if ignored_count > 0 {
            println!(
                "  {} {} file(s) hidden by .repoignore",
                "⊘".dimmed(),
                ignored_count
            );
        }

        let prompt_msg = if amend {
            format!(
                "{} {} unstaged file(s). Add to last commit? [Y/n] {}  ",
                "?".yellow().bold(),
                unstaged,
                "l=list d=diff s=select".dimmed()
            )
        } else {
            format!(
                "{} {} unstaged file(s). Stage all? [y/N] {}  ",
                "?".yellow().bold(),
                unstaged,
                "l=list d=diff s=select".dimmed()
            )
        };

        // Non-interactive: auto-stage all
        if !interactive {
            stage_all(&repo, ignore_set.as_ref())?;
            println!("{} Staged {} file(s)", "✓".green(), unstaged);
        } else {
            let all_files = visible_files;

            loop {
                print!("{}", prompt_msg);
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                let raw = input.trim().to_lowercase();
                // Default: Y for amend (keep existing behavior), N for commit (stage tracked only)
                let choice: &str = if raw.is_empty() {
                    if amend { "y" } else { "n" }
                } else {
                    raw.as_str()
                };

                match choice {
                    "y" => {
                        stage_all(&repo, ignore_set.as_ref())?;
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
                        match run_file_selector(&all_files, repo.path(), &repo)? {
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
                            let tracked: Vec<String> = all_files
                                .iter()
                                .filter(|(_, s)| *s != '?')
                                .map(|(p, _)| p.clone())
                                .collect();
                            if tracked.is_empty() {
                                bail!(
                                    "No tracked changes to commit. Only untracked files present — rerun and answer 'y' to include them, or use 's' to select."
                                );
                            }
                            let count = tracked.len();
                            stage_files(&repo, &tracked)?;
                            println!(
                                "{} Staged {} tracked file(s) {}",
                                "✓".green(),
                                count,
                                "(untracked skipped)".dimmed()
                            );
                            break;
                        }
                    }
                    "c" | "cancel" | "q" => {
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
                                "y=stage all  n=tracked only  c=cancel  l=list  d=diff  s=select"
                                    .dimmed()
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

    // Resolve model: CLI flag > config > provider default
    let model = cli_model.or_else(|| config.commit_model.clone());

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
        let model_display = model.as_deref().map(|m| format!("/{}", m)).unwrap_or_default();
        println!(
            "{} Generating commit message with {}{}...",
            "●".cyan(),
            provider.name().bold(),
            model_display.dimmed()
        );
        let style = config.commit_style.as_deref();
        generate_commit_message(provider, &diff, style, model.as_deref())?
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
                message = generate_commit_message(provider, &diff, style, model.as_deref())?;
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

enum Stage {
    Select,
    Confirm,
}

/// Interactive file selector with checkboxes, live search, and confirmation preview.
fn run_file_selector(
    files: &[(String, char)],
    git_dir: &std::path::Path,
    repo: &Repository,
) -> Result<FileSelection> {
    use std::io::stdout;

    let memory_path = git_dir.join("repo-cli-deselect");
    let remembered = load_deselect_memory(&memory_path);

    let mut selected: Vec<bool> = files
        .iter()
        .map(|(path, _)| !remembered.contains(path))
        .collect();
    let mut visible_idx: Vec<usize> = (0..files.len()).collect();
    let mut cursor_pos: usize = 0; // index into visible_idx
    let mut offset: usize = 0;
    let mut query = String::new();
    let mut search_active = false;
    let mut status_msg = String::new();

    let mut stage = Stage::Select;
    let mut confirm_scroll: u16 = 0;
    let mut confirm_diff = String::new();
    let mut confirm_stats: Vec<crate::git::FileStat> = Vec::new();
    let mut confirm_paths: Vec<String> = Vec::new();

    terminal::enable_raw_mode()?;
    let mut out = stdout();
    out.execute(EnterAlternateScreen)?;
    out.execute(cursor::Hide)?;

    let result = (|| -> Result<FileSelection> {
        loop {
            match stage {
                Stage::Select => draw_selector(
                    &mut out,
                    files,
                    &selected,
                    &visible_idx,
                    cursor_pos,
                    &mut offset,
                    &query,
                    search_active,
                    &status_msg,
                )?,
                Stage::Confirm => draw_confirm(
                    &mut out,
                    &confirm_stats,
                    &confirm_diff,
                    confirm_scroll,
                )?,
            }

            if !event::poll(Duration::from_millis(100))? {
                continue;
            }
            let Event::Key(key) = event::read()? else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            status_msg.clear();

            match stage {
                Stage::Select if search_active => match key.code {
                    KeyCode::Esc => {
                        search_active = false;
                        query.clear();
                        visible_idx = rebuild_filter(files, &query);
                        cursor_pos = 0;
                        offset = 0;
                    }
                    KeyCode::Enter => {
                        search_active = false;
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        visible_idx = rebuild_filter(files, &query);
                        cursor_pos = cursor_pos.min(visible_idx.len().saturating_sub(1));
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                        visible_idx = rebuild_filter(files, &query);
                        cursor_pos = 0;
                        offset = 0;
                    }
                    _ => {}
                },
                Stage::Select => match key.code {
                    KeyCode::Char('/') => {
                        search_active = true;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        cursor_pos = cursor_pos.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if cursor_pos + 1 < visible_idx.len() {
                            cursor_pos += 1;
                        }
                    }
                    KeyCode::PageUp => {
                        let page = visible_rows()?.max(1);
                        cursor_pos = cursor_pos.saturating_sub(page);
                    }
                    KeyCode::PageDown => {
                        let page = visible_rows()?.max(1);
                        cursor_pos =
                            (cursor_pos + page).min(visible_idx.len().saturating_sub(1));
                    }
                    KeyCode::Home => {
                        cursor_pos = 0;
                    }
                    KeyCode::End => {
                        cursor_pos = visible_idx.len().saturating_sub(1);
                    }
                    KeyCode::Char(' ') => {
                        if let Some(&fi) = visible_idx.get(cursor_pos) {
                            selected[fi] = !selected[fi];
                        }
                    }
                    KeyCode::Char('a') => {
                        for &fi in &visible_idx {
                            selected[fi] = true;
                        }
                    }
                    KeyCode::Char('n') => {
                        for &fi in &visible_idx {
                            selected[fi] = false;
                        }
                    }
                    KeyCode::Char('i') => {
                        for &fi in &visible_idx {
                            selected[fi] = !selected[fi];
                        }
                    }
                    KeyCode::Enter => {
                        let paths: Vec<String> = files
                            .iter()
                            .zip(selected.iter())
                            .filter(|(_, &sel)| sel)
                            .map(|((path, _), _)| path.clone())
                            .collect();
                        if paths.is_empty() {
                            status_msg = "no files selected".to_string();
                        } else {
                            match crate::git::get_unstaged_diff_for_paths(repo, &paths) {
                                Ok((diff, stats)) => {
                                    confirm_diff = diff;
                                    confirm_stats = stats;
                                    confirm_paths = paths;
                                    confirm_scroll = 0;
                                    stage = Stage::Confirm;
                                }
                                Err(e) => {
                                    status_msg = format!("diff failed: {}", e);
                                }
                            }
                        }
                    }
                    KeyCode::Esc => {
                        if !query.is_empty() {
                            query.clear();
                            visible_idx = rebuild_filter(files, &query);
                            cursor_pos = 0;
                            offset = 0;
                        } else {
                            return Ok(FileSelection::Cancelled);
                        }
                    }
                    KeyCode::Char('q') => {
                        return Ok(FileSelection::Cancelled);
                    }
                    _ => {}
                },
                Stage::Confirm => match key.code {
                    KeyCode::Enter | KeyCode::Char('y') => {
                        let deselected: Vec<&str> = files
                            .iter()
                            .zip(selected.iter())
                            .filter(|(_, &sel)| !sel)
                            .map(|((path, _), _)| path.as_str())
                            .collect();
                        let _ = save_deselect_memory(&memory_path, &deselected);
                        return Ok(FileSelection::Selected(confirm_paths.clone()));
                    }
                    KeyCode::Esc | KeyCode::Char('b') => {
                        stage = Stage::Select;
                    }
                    KeyCode::Char('q') => {
                        return Ok(FileSelection::Cancelled);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        confirm_scroll = confirm_scroll.saturating_add(1);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        confirm_scroll = confirm_scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown => {
                        confirm_scroll = confirm_scroll.saturating_add(10);
                    }
                    KeyCode::PageUp => {
                        confirm_scroll = confirm_scroll.saturating_sub(10);
                    }
                    _ => {}
                },
            }
        }
    })();

    out.execute(cursor::Show)?;
    out.execute(LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    result
}

fn rebuild_filter(files: &[(String, char)], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..files.len()).collect();
    }
    let q = query.to_lowercase();
    files
        .iter()
        .enumerate()
        .filter(|(_, (path, _))| path.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

fn load_deselect_memory(path: &std::path::Path) -> std::collections::HashSet<String> {
    std::fs::read_to_string(path)
        .map(|s| s.lines().map(|l| l.to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

fn save_deselect_memory(path: &std::path::Path, deselected: &[&str]) -> Result<()> {
    if deselected.is_empty() {
        let _ = std::fs::remove_file(path);
        return Ok(());
    }
    let mut sorted: Vec<&&str> = deselected.iter().collect();
    sorted.sort();
    let content = sorted.iter().map(|s| **s).collect::<Vec<_>>().join("\n");
    std::fs::write(path, content)?;
    Ok(())
}

fn visible_rows() -> Result<usize> {
    let (_, h) = terminal::size()?;
    // reserve: header(1) + blank(1) + blank(1) + count(1) = 4
    Ok((h as usize).saturating_sub(4).max(1))
}

#[allow(clippy::too_many_arguments)]
fn draw_selector(
    out: &mut impl Write,
    files: &[(String, char)],
    selected: &[bool],
    visible_idx: &[usize],
    cursor_pos: usize,
    offset: &mut usize,
    query: &str,
    search_active: bool,
    status_msg: &str,
) -> Result<()> {
    // Reserve 2 header rows + 1 status row = 3
    let (_, h) = terminal::size()?;
    let visible = (h as usize).saturating_sub(3).max(1);

    // Adjust offset so cursor stays in view
    if cursor_pos < *offset {
        *offset = cursor_pos;
    } else if cursor_pos >= *offset + visible {
        *offset = cursor_pos + 1 - visible;
    }
    let max_offset = visible_idx.len().saturating_sub(visible);
    if *offset > max_offset {
        *offset = max_offset;
    }

    out.execute(cursor::MoveTo(0, 0))?;
    out.execute(terminal::Clear(ClearType::All))?;

    let total = files.len();
    let selected_count = selected.iter().filter(|&&s| s).count();

    // Row 1: search/filter line
    if search_active {
        write!(
            out,
            "  {} {}{}\r\n",
            "/".cyan().bold(),
            query,
            "█".cyan(),
        )?;
    } else if !query.is_empty() {
        write!(
            out,
            "  {} {}  {}\r\n",
            "filter:".dimmed(),
            query.cyan(),
            format!("({} matches, Esc to clear)", visible_idx.len()).dimmed(),
        )?;
    } else {
        write!(
            out,
            "  {}\r\n",
            "/: search".dimmed(),
        )?;
    }

    // Row 2: help line + counts
    let bulk_scope = if query.is_empty() { "" } else { " (filtered)" };
    let help = format!(
        "jk=move  Space=toggle  a=all{0}  n=none{0}  i=invert{0}  Enter=preview  Esc=cancel",
        bulk_scope
    );
    let status_tail = if !status_msg.is_empty() {
        format!("  {}", status_msg.yellow())
    } else if visible_idx.len() > visible {
        format!(
            "  [{}–{} of {}]",
            *offset + 1,
            (*offset + visible).min(visible_idx.len()),
            visible_idx.len()
        )
        .dimmed()
        .to_string()
    } else {
        String::new()
    };
    write!(
        out,
        "  {}  {}{}\r\n",
        help.dimmed(),
        format!("{}/{}", selected_count, total).cyan(),
        status_tail,
    )?;

    // Rows: file list (windowed)
    if visible_idx.is_empty() {
        write!(out, "  {}\r\n", "no matches".dimmed())?;
    } else {
        let end = (*offset + visible).min(visible_idx.len());
        for row in *offset..end {
            let fi = visible_idx[row];
            let (path, file_status) = &files[fi];
            let pointer = if row == cursor_pos { ">" } else { " " };
            let checkbox = if selected[fi] { "■" } else { "□" };
            let marker = match file_status {
                '?' => "?".yellow(),
                'M' => "M".cyan(),
                'D' => "D".red(),
                'R' => "R".blue(),
                _ => " ".normal(),
            };
            if row == cursor_pos {
                write!(
                    out,
                    "  {} {} {} {}\r\n",
                    pointer.cyan().bold(),
                    checkbox.green().bold(),
                    marker,
                    path.bold()
                )?;
            } else {
                write!(out, "  {} {} {} {}\r\n", pointer, checkbox.dimmed(), marker, path)?;
            }
        }
    }
    out.flush()?;
    Ok(())
}

fn draw_confirm(
    out: &mut impl Write,
    stats: &[crate::git::FileStat],
    diff: &str,
    scroll: u16,
) -> Result<()> {
    let (_, h) = terminal::size()?;
    let total_h = h as usize;

    out.execute(cursor::MoveTo(0, 0))?;
    out.execute(terminal::Clear(ClearType::All))?;

    // Header
    let total_adds: usize = stats.iter().map(|s| s.adds).sum();
    let total_dels: usize = stats.iter().map(|s| s.dels).sum();
    write!(
        out,
        "  {}  {} files  {} {}\r\n",
        "confirm commit".cyan().bold(),
        stats.len().to_string().cyan(),
        format!("+{}", total_adds).green(),
        format!("-{}", total_dels).red(),
    )?;

    // File stats (cap at 8 rows)
    let max_files = 8usize;
    let shown_files = stats.len().min(max_files);
    for s in stats.iter().take(shown_files) {
        let marker = match s.status {
            '?' | 'A' => "A".green(),
            'M' => "M".cyan(),
            'D' => "D".red(),
            'R' => "R".blue(),
            _ => " ".normal(),
        };
        write!(
            out,
            "  {} {:<50}  {} {}\r\n",
            marker,
            truncate_path(&s.path, 50),
            format!("+{}", s.adds).green(),
            format!("-{}", s.dels).red(),
        )?;
    }
    if stats.len() > max_files {
        write!(
            out,
            "  {}\r\n",
            format!("…{} more file(s)", stats.len() - max_files).dimmed()
        )?;
    }

    // Separator
    write!(out, "  {}\r\n", "─── diff ───".dimmed())?;

    // Diff pane
    // Rows used so far: header(1) + shown_files + (1 if truncated) + 1 (separator) = 2 + shown_files + overflow
    let overflow_row = if stats.len() > max_files { 1 } else { 0 };
    let used = 2 + shown_files + overflow_row;
    let footer = 1usize;
    let diff_rows = total_h.saturating_sub(used + footer).max(1);

    let lines: Vec<&str> = diff.lines().collect();
    let start = (scroll as usize).min(lines.len().saturating_sub(1).max(0));
    let end = (start + diff_rows).min(lines.len());
    for line in &lines[start..end] {
        if line.starts_with("+++") || line.starts_with("---") {
            write!(out, "  {}\r\n", line.dimmed())?;
        } else if line.starts_with('+') {
            write!(out, "  {}\r\n", line.green())?;
        } else if line.starts_with('-') {
            write!(out, "  {}\r\n", line.red())?;
        } else if line.starts_with("@@") {
            write!(out, "  {}\r\n", line.cyan())?;
        } else {
            write!(out, "  {}\r\n", line)?;
        }
    }

    // Footer
    let total_lines = lines.len();
    let scroll_hint = if total_lines > diff_rows {
        format!(
            " [{}–{}/{}]",
            start + 1,
            end.min(total_lines),
            total_lines
        )
    } else {
        String::new()
    };
    write!(
        out,
        "  {}{}\r\n",
        "Enter/y=commit  Esc/b=back  j/k=scroll  PgUp/PgDn=page  q=cancel".dimmed(),
        scroll_hint.dimmed(),
    )?;

    out.flush()?;
    Ok(())
}

fn truncate_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        format!("{:<width$}", path, width = max)
    } else {
        let keep = max.saturating_sub(1);
        let tail = &path[path.len().saturating_sub(keep)..];
        format!("…{}", tail)
    }
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
