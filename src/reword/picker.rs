use anyhow::Result;
use colored::Colorize;

use crate::models::{format_relative_time, CommitInfo};

pub fn pick_commits(commits: &[CommitInfo]) -> Result<Vec<usize>> {
    use std::io::{self, Write};

    let is_merge: Vec<bool> = commits.iter().map(|c| c.parents.len() > 1).collect();

    // display numbered list
    println!("\n{}", "SELECT COMMITS TO REWORD".bold());
    println!("{}", "─".repeat(60).dimmed());

    for (i, c) in commits.iter().enumerate() {
        let num = format!("{:>3}", i + 1);
        let merge_tag = if is_merge[i] {
            " (merge)".dimmed().to_string()
        } else {
            String::new()
        };

        println!(
            "  {} {} {} {}{}",
            num.cyan(),
            c.short_id.yellow(),
            c.message,
            format_relative_time(&c.time).dimmed(),
            merge_tag,
        );
    }

    println!("{}", "─".repeat(60).dimmed());
    println!(
        "  {} toggle: 1, 1-5, 1,3,5 | a=all n=none Enter=confirm",
        "?".cyan()
    );

    let mut selected = vec![false; commits.len()];

    loop {
        print_selection_summary(&selected, commits);
        print!("  > ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            break;
        }

        match input {
            "a" => {
                for i in 0..commits.len() {
                    selected[i] = !is_merge[i];
                }
            }
            "n" => {
                for s in &mut selected {
                    *s = false;
                }
            }
            _ => {
                // parse number, range, or comma-separated
                if let Some(indices) = parse_selection(input, commits.len()) {
                    for i in indices {
                        if is_merge[i] {
                            eprintln!(
                                "  {} commit {} is a merge, skipping",
                                "⚠".yellow(),
                                i + 1
                            );
                        } else {
                            selected[i] = !selected[i];
                        }
                    }
                } else {
                    eprintln!("  {} invalid input", "!".red());
                }
            }
        }
    }

    let skipped_merges = selected
        .iter()
        .enumerate()
        .filter(|(i, &s)| s && is_merge[*i])
        .count();
    if skipped_merges > 0 {
        eprintln!(
            "{} skipping {} merge commit(s)",
            "⚠".yellow(),
            skipped_merges
        );
    }

    let result: Vec<usize> = selected
        .iter()
        .enumerate()
        .filter(|(i, &s)| s && !is_merge[*i])
        .map(|(i, _)| i)
        .collect();

    Ok(result)
}

fn print_selection_summary(selected: &[bool], _commits: &[CommitInfo]) {
    let count = selected.iter().filter(|&&s| s).count();
    if count == 0 {
        return;
    }

    let nums: Vec<String> = selected
        .iter()
        .enumerate()
        .filter(|(_, &s)| s)
        .map(|(i, _)| format!("{}", i + 1))
        .collect();

    print!(
        "  {} selected: {} ",
        format!("[{}]", count).green(),
        nums.join(", ").dimmed()
    );
    // clear line
    println!();
}

fn parse_selection(input: &str, max: usize) -> Option<Vec<usize>> {
    let mut indices = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.contains('-') {
            let bounds: Vec<&str> = part.split('-').collect();
            if bounds.len() != 2 {
                return None;
            }
            let start: usize = bounds[0].trim().parse().ok()?;
            let end: usize = bounds[1].trim().parse().ok()?;
            if start == 0 || end == 0 || start > max || end > max {
                return None;
            }
            let (lo, hi) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            for i in lo..=hi {
                indices.push(i - 1); // 0-indexed
            }
        } else {
            let n: usize = part.parse().ok()?;
            if n == 0 || n > max {
                return None;
            }
            indices.push(n - 1);
        }
    }

    Some(indices)
}
