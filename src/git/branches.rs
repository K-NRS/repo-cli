use anyhow::{Context, Result};
use chrono::{Local, TimeZone};
use git2::{BranchType, Repository};

use crate::models::{BranchInfo, RemoteBranchInfo, UpstreamInfo};

pub fn get_current_branch(repo: &Repository) -> Result<BranchInfo> {
    let head = repo.head().context("Failed to get HEAD")?;

    let name = if head.is_branch() {
        head.shorthand().unwrap_or("HEAD").to_string()
    } else {
        // Detached HEAD - show short commit hash
        let oid = head.target().context("No target for HEAD")?;
        format!("HEAD@{}", &oid.to_string()[..7])
    };

    let upstream = get_upstream_info(repo, &name);
    let tip_commit = head.target();

    Ok(BranchInfo {
        name,
        is_head: true,
        upstream,
        tip_commit,
    })
}

fn get_upstream_info(repo: &Repository, branch_name: &str) -> Option<UpstreamInfo> {
    let branch = repo.find_branch(branch_name, BranchType::Local).ok()?;
    let upstream = branch.upstream().ok()?;
    let upstream_name = upstream.name().ok()??.to_string();

    let (ahead, behind) = repo
        .graph_ahead_behind(
            branch.get().target()?,
            upstream.get().target()?,
        )
        .ok()?;

    Some(UpstreamInfo {
        name: upstream_name,
        ahead,
        behind,
    })
}

pub fn get_local_branches(repo: &Repository) -> Result<Vec<BranchInfo>> {
    let mut branches = Vec::new();
    let head_ref = repo.head().ok();
    let head_name = head_ref.as_ref().and_then(|h| h.shorthand()).unwrap_or("");

    for branch_result in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch_result?;
        let name = branch.name()?.unwrap_or("").to_string();
        let is_head = name == head_name;
        let upstream = get_upstream_info(repo, &name);
        let tip_commit = branch.get().target();

        branches.push(BranchInfo {
            name,
            is_head,
            upstream,
            tip_commit,
        });
    }

    // Sort: HEAD first, then alphabetically
    branches.sort_by(|a, b| {
        if a.is_head {
            std::cmp::Ordering::Less
        } else if b.is_head {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    Ok(branches)
}

pub fn get_remote_branches(repo: &Repository) -> Result<Vec<RemoteBranchInfo>> {
    let mut branches = Vec::new();

    for branch_result in repo.branches(Some(BranchType::Remote))? {
        let (branch, _) = branch_result?;
        let full_name = branch.name()?.unwrap_or("").to_string();

        // Skip HEAD references
        if full_name.ends_with("/HEAD") {
            continue;
        }

        // Parse remote/branch format
        let parts: Vec<&str> = full_name.splitn(2, '/').collect();
        let (remote, short_name) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            ("origin".to_string(), full_name.clone())
        };

        // Get last commit info
        let (last_commit_time, last_commit_message) = if let Some(oid) = branch.get().target() {
            if let Ok(commit) = repo.find_commit(oid) {
                let time = commit.time();
                let dt = Local.timestamp_opt(time.seconds(), 0).single().unwrap_or_else(Local::now);
                let msg = commit.summary().unwrap_or("").to_string();
                (dt, msg)
            } else {
                (Local::now(), String::new())
            }
        } else {
            (Local::now(), String::new())
        };

        branches.push(RemoteBranchInfo {
            name: full_name,
            remote,
            short_name,
            last_commit_time,
            last_commit_message,
        });
    }

    // Sort by most recent
    branches.sort_by(|a, b| b.last_commit_time.cmp(&a.last_commit_time));

    Ok(branches)
}
