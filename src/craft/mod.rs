pub mod actions;
mod execute;
pub mod split;
mod tui;

use anyhow::{bail, Result};
use colored::Colorize;
use git2::Repository;

use crate::models::CommitInfo;
use tui::{run_craft_tui, CraftResult};

pub struct CraftArgs {
    pub count: usize,
    pub last: Option<usize>,
}

pub fn run_craft(repo: &Repository, args: CraftArgs) -> Result<()> {
    validate_state(repo)?;

    let commits = load_commits(repo, args.count)?;
    if commits.is_empty() {
        println!("{} no commits found", "!".yellow());
        return Ok(());
    }

    let result = run_craft_tui(commits.clone(), repo)?;

    match result {
        CraftResult::Execute(entries, hunks_cache) => {
            let action_count = entries.iter().filter(|e| !matches!(e.action, actions::RebaseAction::Pick)).count();
            if action_count == 0 {
                println!("{} no changes to apply", "!".yellow());
                return Ok(());
            }

            // Only warn about pushed commits at execute time, checking modified commits only
            let modified_indices: Vec<usize> = entries
                .iter()
                .filter(|e| !matches!(e.action, actions::RebaseAction::Pick))
                .map(|e| e.original_idx)
                .collect();
            warn_pushed_commits(repo, &commits, &modified_indices);

            let repo_path = repo
                .workdir()
                .unwrap_or_else(|| repo.path())
                .to_path_buf();

            execute::execute_craft_plan(&repo_path, &commits, &entries, &hunks_cache)?;
            println!("{} crafted {} action(s)", "done".green(), action_count);
        }
        CraftResult::Cancel => {
            println!("{}", "cancelled".dimmed());
        }
    }

    Ok(())
}

fn validate_state(repo: &Repository) -> Result<()> {
    if repo.head_detached()? {
        bail!("detached HEAD — cannot craft");
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

fn warn_pushed_commits(repo: &Repository, commits: &[CommitInfo], modified: &[usize]) {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return,
    };
    let branch_name = head.shorthand().unwrap_or("");
    let upstream_ref = format!("refs/remotes/origin/{}", branch_name);

    if let Ok(reference) = repo.find_reference(&upstream_ref) {
        if let Some(upstream_oid) = reference.target() {
            let pushed_count = modified.iter().filter(|&&i| {
                let c = &commits[i];
                repo.graph_descendant_of(upstream_oid, c.id).unwrap_or(false)
                    || upstream_oid == c.id
            }).count();

            if pushed_count > 0 {
                eprintln!(
                    "{} {} modified commit(s) already pushed — you'll need `git push --force` after",
                    "!".yellow(),
                    pushed_count,
                );
            }
        }
    }
}
