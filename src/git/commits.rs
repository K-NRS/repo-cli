use anyhow::Result;
use chrono::{Local, TimeZone};
use git2::Repository;

use crate::models::CommitInfo;

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
