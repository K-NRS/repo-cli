use anyhow::Result;
use chrono::{Duration, Local, TimeZone};
use git2::Repository;

use crate::models::{BranchInfo, BranchTreeNode};

pub fn build_branch_tree(
    repo: &Repository,
    branches: &[BranchInfo],
    stale_days: u64,
) -> Result<Vec<BranchTreeNode>> {
    let now = Local::now();
    let stale_threshold = now - Duration::days(stale_days as i64);

    let main_idx = branches
        .iter()
        .position(|b| b.is_head)
        .or_else(|| {
            branches
                .iter()
                .position(|b| b.name == "main" || b.name == "master")
        })
        .unwrap_or(0);

    let mut nodes: Vec<BranchTreeNode> = branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let last_activity = b.tip_commit.and_then(|oid| {
                repo.find_commit(oid).ok().map(|c| {
                    Local
                        .timestamp_opt(c.time().seconds(), 0)
                        .single()
                        .unwrap_or_else(Local::now)
                })
            });

            let is_stale = last_activity
                .map(|t| t < stale_threshold)
                .unwrap_or(false);

            let is_merged = is_branch_merged(repo, b, branches.get(main_idx));
            let unique_commits = count_unique_commits(repo, b, branches.get(main_idx));

            BranchTreeNode {
                branch: b.clone(),
                depth: if i == main_idx { 0 } else { 1 },
                children: Vec::new(),
                parent: if i == main_idx {
                    None
                } else {
                    Some(main_idx)
                },
                is_merged,
                is_stale,
                last_activity,
                unique_commits,
            }
        })
        .collect();

    if !nodes.is_empty() {
        let child_indices: Vec<usize> = (0..nodes.len()).filter(|&i| i != main_idx).collect();
        nodes[main_idx].children = child_indices;
    }

    Ok(nodes)
}

fn is_branch_merged(repo: &Repository, branch: &BranchInfo, main: Option<&BranchInfo>) -> bool {
    let Some(main) = main else { return false };
    let Some(branch_oid) = branch.tip_commit else {
        return false;
    };
    let Some(main_oid) = main.tip_commit else {
        return false;
    };

    if branch_oid == main_oid {
        return true;
    }

    repo.graph_descendant_of(main_oid, branch_oid)
        .unwrap_or(false)
}

fn count_unique_commits(
    repo: &Repository,
    branch: &BranchInfo,
    main: Option<&BranchInfo>,
) -> usize {
    let Some(main) = main else { return 0 };
    let Some(branch_oid) = branch.tip_commit else {
        return 0;
    };
    let Some(main_oid) = main.tip_commit else {
        return 0;
    };

    repo.graph_ahead_behind(branch_oid, main_oid)
        .map(|(ahead, _)| ahead)
        .unwrap_or(0)
}
