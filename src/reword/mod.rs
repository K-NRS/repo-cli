mod picker;
mod rebase;

use anyhow::{bail, Result};
use colored::Colorize;
use git2::Repository;

use crate::models::{format_relative_time, CommitInfo};

pub struct RewordArgs {
    pub last: Option<usize>,
    pub all: bool,
    pub count: usize,
    pub editor: bool,
}

pub fn run_reword(repo: &Repository, args: RewordArgs) -> Result<()> {
    validate_state(repo)?;

    let commits = load_commits(repo, args.count)?;
    if commits.is_empty() {
        println!("{} no commits found", "!".yellow());
        return Ok(());
    }

    let selected = select_commits(&commits, &args)?;
    if selected.is_empty() {
        println!("{} no commits selected", "!".yellow());
        return Ok(());
    }

    let messages = collect_new_messages(&commits, &selected, args.editor)?;
    if messages.is_empty() {
        println!("{} no messages changed", "!".yellow());
        return Ok(());
    }

    // Build SHAs from messages (only commits with actual new messages)
    let reword_shas: Vec<String> = messages
        .iter()
        .map(|(sha, _)| sha[..7].to_string())
        .collect();

    // Find indices of commits being reworded to determine base
    let reword_indices: Vec<usize> = commits
        .iter()
        .enumerate()
        .filter(|(_, c)| messages.iter().any(|(sha, _)| c.id.to_string() == *sha))
        .map(|(i, _)| i)
        .collect();

    warn_pushed_commits(repo, &commits, &reword_indices)?;

    let has_root = reword_indices.iter().any(|&i| commits[i].parents.is_empty());
    let base_sha = if has_root {
        None
    } else {
        let oldest_idx = *reword_indices.iter().max().unwrap();
        Some(commits[oldest_idx].parents[0].to_string())
    };

    let repo_path = repo
        .workdir()
        .unwrap_or_else(|| repo.path())
        .to_path_buf();

    rebase::run_interactive_rebase(&repo_path, base_sha.as_deref(), &reword_shas, &messages)?;

    println!("{} reworded {} commit(s)", "✓".green(), messages.len());
    Ok(())
}

fn validate_state(repo: &Repository) -> Result<()> {
    if repo.head_detached()? {
        bail!("detached HEAD — cannot reword");
    }

    let statuses = repo.statuses(None)?;
    let dirty = statuses.iter().any(|s| {
        let st = s.status();
        st.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::WT_MODIFIED
                | git2::Status::WT_NEW
                | git2::Status::WT_DELETED,
        )
    });
    if dirty {
        bail!("dirty working tree — commit or stash changes first");
    }

    Ok(())
}

fn load_commits(repo: &Repository, limit: usize) -> Result<Vec<CommitInfo>> {
    use chrono::{Local, TimeZone};

    let head = repo.head()?;
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TIME)?;
    revwalk.push(head.target().unwrap())?;

    let mut commits = Vec::new();
    for (count, oid_result) in revwalk.enumerate() {
        if count >= limit {
            break;
        }
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;
        let time = commit.time();
        let dt = Local
            .timestamp_opt(time.seconds(), 0)
            .single()
            .unwrap_or_else(Local::now);
        let parents: Vec<_> = commit.parent_ids().collect();

        commits.push(CommitInfo {
            id: oid,
            short_id: oid.to_string()[..7].to_string(),
            message: commit.summary().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("").to_string(),
            time: dt,
            parents,
        });
    }

    Ok(commits)
}

fn select_commits(commits: &[CommitInfo], args: &RewordArgs) -> Result<Vec<usize>> {
    if args.all {
        let mut indices: Vec<usize> = (0..commits.len())
            .filter(|&i| !is_merge(&commits[i]))
            .collect();
        let skipped = commits.len() - indices.len();
        if skipped > 0 {
            eprintln!(
                "{} skipping {} merge commit(s)",
                "⚠".yellow(),
                skipped
            );
        }
        indices.sort();
        return Ok(indices);
    }

    if let Some(n) = args.last {
        let n = n.min(commits.len());
        let mut indices: Vec<usize> = (0..n)
            .filter(|&i| !is_merge(&commits[i]))
            .collect();
        let skipped = n - indices.len();
        if skipped > 0 {
            eprintln!(
                "{} skipping {} merge commit(s)",
                "⚠".yellow(),
                skipped
            );
        }
        indices.sort();
        return Ok(indices);
    }

    // interactive picker
    picker::pick_commits(commits)
}

fn is_merge(commit: &CommitInfo) -> bool {
    commit.parents.len() > 1
}

fn collect_new_messages(
    commits: &[CommitInfo],
    selected: &[usize],
    use_editor: bool,
) -> Result<Vec<(String, String)>> {
    // oldest-first ordering for editing
    let mut ordered: Vec<usize> = selected.to_vec();
    ordered.sort();
    ordered.reverse(); // oldest first (highest index = oldest)

    let mut mappings = Vec::new();

    for &idx in &ordered {
        let c = &commits[idx];
        println!(
            "\n  {} {} {}",
            c.short_id.yellow(),
            c.message,
            format_relative_time(&c.time).dimmed()
        );

        let new_msg = if use_editor {
            edit_with_editor(&c.message)?
        } else {
            prompt_inline(&c.message)?
        };

        if let Some(msg) = new_msg {
            mappings.push((c.id.to_string(), msg));
        }
    }

    Ok(mappings)
}

fn prompt_inline(current: &str) -> Result<Option<String>> {
    use std::io::{self, Write};

    print!("  new message (Enter=keep, e=editor): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        return Ok(None);
    }

    if input == "e" {
        return edit_with_editor(current);
    }

    Ok(Some(input.to_string()))
}

fn edit_with_editor(current: &str) -> Result<Option<String>> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let tmp = std::env::temp_dir().join(format!("repo-reword-{}.txt", std::process::id()));
    std::fs::write(&tmp, current)?;

    let status = std::process::Command::new(&editor)
        .arg(&tmp)
        .status()?;

    if !status.success() {
        std::fs::remove_file(&tmp).ok();
        bail!("editor exited with non-zero status");
    }

    let new_msg = std::fs::read_to_string(&tmp)?.trim().to_string();
    std::fs::remove_file(&tmp).ok();

    if new_msg == current || new_msg.is_empty() {
        return Ok(None);
    }

    Ok(Some(new_msg))
}

fn warn_pushed_commits(
    repo: &Repository,
    commits: &[CommitInfo],
    selected: &[usize],
) -> Result<()> {
    // check if current branch has an upstream
    let head = repo.head()?;
    let branch_name = head.shorthand().unwrap_or("");
    let upstream_ref = format!("refs/remotes/origin/{}", branch_name);

    if repo.find_reference(&upstream_ref).is_ok() {
        // some selected commits may be pushed
        let upstream_oid = repo.find_reference(&upstream_ref)?.target();
        if let Some(upstream_oid) = upstream_oid {
            let has_pushed = selected.iter().any(|&i| {
                repo.graph_descendant_of(upstream_oid, commits[i].id).unwrap_or(false)
                    || upstream_oid == commits[i].id
            });

            if has_pushed {
                use std::io::{self, Write};
                eprint!(
                    "{} selected commits are already pushed — force-push needed. continue? [y/N] ",
                    "⚠".yellow()
                );
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    bail!("aborted");
                }
            }
        }
    }

    Ok(())
}
