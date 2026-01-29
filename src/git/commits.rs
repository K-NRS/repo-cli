use anyhow::Result;
use chrono::{Local, TimeZone};
use git2::{BranchType, Repository};
use std::collections::HashSet;

use crate::models::{BranchCommitCount, CommitInfo};

pub fn get_recent_commits(repo: &Repository, limit: usize) -> Result<Vec<CommitInfo>> {
    let mut commits = Vec::new();

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    // Push all local branches
    for branch in repo.branches(Some(git2::BranchType::Local))? {
        if let Ok((branch, _)) = branch {
            if let Some(oid) = branch.get().target() {
                let _ = revwalk.push(oid);
            }
        }
    }

    // Push all remote branches
    for branch in repo.branches(Some(git2::BranchType::Remote))? {
        if let Ok((branch, _)) = branch {
            if let Some(oid) = branch.get().target() {
                let _ = revwalk.push(oid);
            }
        }
    }

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

pub fn get_total_commit_count(repo: &Repository) -> Result<usize> {
    let mut seen = HashSet::new();
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    // Push all local branches
    for branch in repo.branches(Some(BranchType::Local))? {
        if let Ok((branch, _)) = branch {
            if let Some(oid) = branch.get().target() {
                let _ = revwalk.push(oid);
            }
        }
    }

    // Push all remote branches
    for branch in repo.branches(Some(BranchType::Remote))? {
        if let Ok((branch, _)) = branch {
            if let Some(oid) = branch.get().target() {
                let _ = revwalk.push(oid);
            }
        }
    }

    // Count unique commits
    for oid_result in revwalk {
        if let Ok(oid) = oid_result {
            seen.insert(oid);
        }
    }

    Ok(seen.len())
}

pub fn get_branch_commit_counts(repo: &Repository) -> Result<Vec<BranchCommitCount>> {
    let mut branch_counts = Vec::new();

    // Get local branches only
    for branch_result in repo.branches(Some(BranchType::Local))? {
        if let Ok((branch, _)) = branch_result {
            if let Some(name) = branch.name()? {
                if let Some(oid) = branch.get().target() {
                    let count = count_commits_from(repo, oid)?;
                    branch_counts.push(BranchCommitCount {
                        name: name.to_string(),
                        count,
                    });
                }
            }
        }
    }

    // Sort by count descending, then by name
    branch_counts.sort_by(|a, b| {
        b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name))
    });

    Ok(branch_counts)
}

fn count_commits_from(repo: &Repository, oid: git2::Oid) -> Result<usize> {
    let mut seen = HashSet::new();
    let mut revwalk = repo.revwalk()?;
    revwalk.push(oid)?;

    for oid_result in revwalk {
        if let Ok(oid) = oid_result {
            seen.insert(oid);
        }
    }

    Ok(seen.len())
}
