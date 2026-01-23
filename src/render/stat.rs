use colored::Colorize;

use crate::models::{format_relative_time, RepoSummary};
use crate::render::graph::render_simple_graph;

pub fn render_static(summary: &RepoSummary, show_graph: bool, use_color: bool, show_stashes: bool) {
    if !use_color {
        colored::control::set_override(false);
    }

    render_header(summary);
    render_status(summary, !show_stashes);
    println!();
    render_recent_commits(summary);

    if show_graph {
        println!();
        render_simple_graph(summary);
    }

    if !summary.remote_branches.is_empty() {
        println!();
        render_remote_branches(summary);
    }

    if show_stashes && !summary.stashes.is_empty() {
        println!();
        render_stashes(summary);
    }
}

fn render_header(summary: &RepoSummary) {
    let branch = &summary.current_branch;
    let branch_display = format!("{}", branch.name).cyan().bold();

    print!("{} ON: {}", "📍".to_string(), branch_display);

    if let Some(ref upstream) = branch.upstream {
        let mut parts = Vec::new();
        if upstream.ahead > 0 {
            parts.push(format!("{}↑", upstream.ahead).green().to_string());
        }
        if upstream.behind > 0 {
            parts.push(format!("{}↓", upstream.behind).red().to_string());
        }

        let remote_name = upstream
            .name
            .split('/')
            .next()
            .unwrap_or("origin");

        if parts.is_empty() {
            print!(" ({})", remote_name.dimmed());
        } else {
            print!(" ({} {})", parts.join(" "), remote_name.dimmed());
        }
    }

    if summary.github_stars.is_some() || summary.github_forks.is_some() {
        print!(" ");
        if let Some(stars) = summary.github_stars {
            print!(" {}{}", "★".yellow(), stars);
        }
        if let Some(forks) = summary.github_forks {
            print!(" {}{}", "⑂".dimmed(), forks);
        }
    }

    println!();
}

fn render_status(summary: &RepoSummary, show_stash_count: bool) {
    let status = &summary.status;
    let stash_count = summary.stashes.len();

    if status.is_clean() && stash_count == 0 {
        println!("   {}", "working tree clean".dimmed());
        return;
    }

    let mut parts = Vec::new();

    let total_changed = status.total_changes();
    if total_changed > 0 {
        parts.push(format!(
            "{} file{} changed",
            total_changed,
            if total_changed == 1 { "" } else { "s" }
        ));
    }

    if status.untracked > 0 {
        parts.push(format!("{} untracked", status.untracked));
    }

    if status.conflicted > 0 {
        parts.push(format!("{} conflicted", status.conflicted).red().to_string());
    }

    if status.staged > 0 {
        parts.push(format!("{} staged", status.staged).green().to_string());
    }

    if show_stash_count && stash_count > 0 {
        parts.push(format!("{} stash{}", stash_count, if stash_count == 1 { "" } else { "es" }).dimmed().to_string());
    }

    if parts.is_empty() {
        println!("   {}", "working tree clean".dimmed());
    } else {
        println!("   {}", parts.join(", "));
    }
}

fn render_recent_commits(summary: &RepoSummary) {
    println!("{}", "RECENT".bold());

    for commit in &summary.recent_commits {
        let time = format_relative_time(&commit.time);
        let time_padded = format!("{:>4}", time);
        let author_short = commit.author.split_whitespace().next().unwrap_or(&commit.author);

        println!(
            "   {} {}  {}  {}",
            "●".yellow(),
            time_padded.dimmed(),
            truncate(&commit.message, 50),
            author_short.dimmed()
        );
    }
}

fn render_remote_branches(summary: &RepoSummary) {
    println!("{}", "REMOTE".bold());

    for branch in summary.remote_branches.iter().take(5) {
        let time = format_relative_time(&branch.last_commit_time);
        let time_padded = format!("{:>4}", time);
        let author_short = branch.last_commit_author.split_whitespace().next().unwrap_or(&branch.last_commit_author);

        println!(
            "   {:<25} {}  \"{}\"  {}",
            branch.name.blue(),
            time_padded.dimmed(),
            truncate(&branch.last_commit_message, 30),
            author_short.dimmed()
        );
    }

    if summary.remote_branches.len() > 5 {
        println!(
            "   {}",
            format!("... and {} more", summary.remote_branches.len() - 5).dimmed()
        );
    }
}

fn render_stashes(summary: &RepoSummary) {
    println!("{} ({})", "STASHES".bold(), summary.stashes.len());

    for stash in &summary.stashes {
        println!("   {}: {}", stash.index, truncate(&stash.message, 40));
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
