use colored::Colorize;
use std::collections::HashMap;

use crate::models::RepoSummary;

pub fn render_simple_graph(summary: &RepoSummary) {
    println!("{}", "TREE".bold());

    if summary.local_branches.is_empty() {
        println!("   {}", "no branches".dimmed());
        return;
    }

    // Build a simple visual representation
    // This shows branch relationships based on commits

    let current = &summary.current_branch.name;

    // Collect branches with their tip commit times for sorting
    let mut branches_with_time: Vec<_> = summary
        .local_branches
        .iter()
        .filter_map(|b| {
            b.tip_commit.and_then(|_| {
                // Find last commit time from recent commits if it's the current branch
                if b.name == *current && !summary.recent_commits.is_empty() {
                    Some((b, summary.recent_commits[0].time))
                } else {
                    // For other branches, we'd need to look up their commits
                    // For simplicity, we'll just use a placeholder
                    Some((b, chrono::Local::now()))
                }
            })
        })
        .collect();

    // Sort: current branch first, then others
    branches_with_time.sort_by(|(a, _), (b, _)| {
        if a.is_head {
            std::cmp::Ordering::Less
        } else if b.is_head {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    // Find common ancestors to determine branch relationships
    // For now, render a simplified tree

    // Track which column each branch is at
    let mut branch_columns: HashMap<String, usize> = HashMap::new();
    let mut next_column = 0;

    // Render main/master first if it exists
    let main_branch = summary
        .local_branches
        .iter()
        .find(|b| b.name == "main" || b.name == "master");

    if let Some(main) = main_branch {
        render_branch_line(&main.name, 0, main.is_head, current);
        branch_columns.insert(main.name.clone(), 0);
        next_column = 1;
    }

    // Render other branches
    for (branch, _) in &branches_with_time {
        if branch_columns.contains_key(&branch.name) {
            continue;
        }

        let is_main_child = main_branch.is_some();
        if is_main_child && next_column > 0 {
            // Show as child of main
            render_child_branch_line(&branch.name, next_column, branch.is_head, current);
        } else {
            render_branch_line(&branch.name, next_column, branch.is_head, current);
        }

        branch_columns.insert(branch.name.clone(), next_column);
        next_column += 1;
    }
}

fn render_branch_line(name: &str, _column: usize, is_head: bool, current: &str) {
    let marker = if is_head { "← YOU" } else { "" };
    let branch_display = if name == current {
        format!("{}", name).cyan().bold().to_string()
    } else {
        name.to_string()
    };

    let commits = "●──●──●";
    println!(
        "   {} {} {}",
        branch_display,
        commits.yellow(),
        marker.green().bold()
    );
}

fn render_child_branch_line(name: &str, indent: usize, is_head: bool, current: &str) {
    let marker = if is_head { "← YOU" } else { "" };
    let branch_display = if name == current {
        format!("{}", name).cyan().bold().to_string()
    } else {
        name.to_string()
    };

    let prefix = "   ".repeat(indent);
    let fork = "└──";
    let commits = "●──●";

    println!(
        "   {}{}{}  {} {} {}",
        prefix,
        "│".dimmed(),
        "",
        "",
        "",
        ""
    );
    println!(
        "   {}{} {} {} {}",
        prefix,
        fork.dimmed(),
        branch_display,
        commits.yellow(),
        marker.green().bold()
    );
}

pub fn render_full_graph(summary: &RepoSummary) {
    println!("{}", "TREE (full)".bold());

    // For full graph mode, we'd need to build actual commit graph
    // This is a more complex algorithm - for now, show extended simple graph

    let commits = &summary.recent_commits;
    if commits.is_empty() {
        println!("   {}", "no commits".dimmed());
        return;
    }

    for (i, commit) in commits.iter().enumerate() {
        let is_merge = commit.parents.len() > 1;
        let symbol = if is_merge { "◆" } else { "●" };

        let prefix = if i == commits.len() - 1 { " " } else { "│" };

        println!(
            "   {} {} {} {}",
            symbol.yellow(),
            commit.short_id.dimmed(),
            commit.message,
            if i == 0 {
                format!("({})", summary.current_branch.name).cyan().to_string()
            } else {
                String::new()
            }
        );

        if i < commits.len() - 1 {
            println!("   {}", prefix.dimmed());
        }
    }
}
