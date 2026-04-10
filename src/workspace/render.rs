use colored::Colorize;

use crate::models::format_relative_time;
use crate::workspace::filter::{filter_repos, WorkspaceFilter};
use crate::workspace::{RepoSnapshot, WorkspaceSummary};

pub fn render_workspace(summary: &WorkspaceSummary, filters: &[WorkspaceFilter], use_color: bool) {
    if !use_color {
        colored::control::set_override(false);
    }

    render_header(summary);

    if summary.repos.is_empty() {
        println!("   {}", "no git repositories found".dimmed());
        render_errors(summary);
        return;
    }

    let visible: Vec<&RepoSnapshot> = if filters.is_empty() {
        summary.repos.iter().collect()
    } else {
        filter_repos(&summary.repos, filters)
    };

    if visible.is_empty() {
        println!("   {}", "no repos match filters".dimmed());
        return;
    }

    let name_width = visible
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(10)
        .min(28);

    let branch_width = visible
        .iter()
        .map(|r| r.branch.chars().count())
        .max()
        .unwrap_or(10)
        .min(24);

    println!();
    for repo in &visible {
        render_card(repo, name_width, branch_width);
    }

    render_errors(summary);
}

fn render_header(summary: &WorkspaceSummary) {
    let source = summary.source.display();
    let count = summary.repos.len();
    println!(
        "{} {} · {} repo{}",
        "▣".cyan().bold(),
        source.bold(),
        count,
        if count == 1 { "" } else { "s" }
    );
}

fn render_card(repo: &RepoSnapshot, name_width: usize, branch_width: usize) {
    let name_cell = pad(&repo.name, name_width);
    let branch_cell = pad(&repo.branch, branch_width);

    let dot = if repo.is_dirty() {
        "●".yellow()
    } else if repo.stale {
        "●".bright_black()
    } else {
        "●".green()
    };

    let upstream = format_upstream(repo);
    let status = format_status(repo);
    let activity = repo
        .last_activity
        .as_ref()
        .map(format_relative_time)
        .unwrap_or_else(|| "—".to_string());

    let last_msg = repo
        .recent_commits
        .first()
        .map(|c| truncate(&c.message, 48))
        .unwrap_or_default();

    println!(
        "   {}  {}  {}  {}{}  {}  {}",
        dot,
        name_cell.bold(),
        branch_cell.cyan(),
        status,
        upstream,
        format!("{:>4}", activity).dimmed(),
        last_msg.dimmed()
    );
}

fn format_upstream(repo: &RepoSnapshot) -> String {
    let Some(up) = &repo.upstream else {
        return String::new();
    };
    let mut parts = Vec::new();
    if up.ahead > 0 {
        parts.push(format!("{}↑", up.ahead).green().to_string());
    }
    if up.behind > 0 {
        parts.push(format!("{}↓", up.behind).red().to_string());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" {}", parts.join(""))
    }
}

fn format_status(repo: &RepoSnapshot) -> String {
    let s = &repo.status;
    if s.is_clean() {
        return pad("clean", 12).dimmed().to_string();
    }
    let mut chunks = Vec::new();
    if s.staged > 0 {
        chunks.push(format!("{}s", s.staged).green().to_string());
    }
    if s.modified > 0 {
        chunks.push(format!("{}m", s.modified).yellow().to_string());
    }
    if s.untracked > 0 {
        chunks.push(format!("{}?", s.untracked).bright_black().to_string());
    }
    if s.conflicted > 0 {
        chunks.push(format!("{}!", s.conflicted).red().to_string());
    }
    let joined = chunks.join(" ");
    format!("{:<12}", strip_colors_len_pad(&joined, 12))
}

fn strip_colors_len_pad(s: &str, width: usize) -> String {
    // colored strings include ANSI codes; visible len is tricky.
    // Just append spaces to hit width; slightly imperfect but fine for terminals.
    let visible = strip_ansi(s);
    let pad = width.saturating_sub(visible.chars().count());
    format!("{}{}", s, " ".repeat(pad))
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_esc = false;
    for c in s.chars() {
        if in_esc {
            if c == 'm' {
                in_esc = false;
            }
            continue;
        }
        if c == '\x1b' {
            in_esc = true;
            continue;
        }
        out.push(c);
    }
    out
}

fn pad(s: &str, width: usize) -> String {
    let visible = s.chars().count();
    if visible >= width {
        s.chars().take(width).collect()
    } else {
        format!("{}{}", s, " ".repeat(width - visible))
    }
}

fn truncate(s: &str, max: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    if c.len() <= max {
        s.to_string()
    } else {
        let mut out: String = c.into_iter().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn render_errors(summary: &WorkspaceSummary) {
    if summary.errors.is_empty() {
        return;
    }
    println!();
    println!("{} ({})", "ERRORS".red().bold(), summary.errors.len());
    for (path, err) in summary.errors.iter().take(5) {
        println!("   {} {}", path.display().to_string().dimmed(), err.dimmed());
    }
    if summary.errors.len() > 5 {
        println!(
            "   {}",
            format!("... and {} more", summary.errors.len() - 5).dimmed()
        );
    }
}
