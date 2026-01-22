mod repo;
mod branches;
mod commits;
mod status;
mod stash;
mod diff;
mod commit_ops;

pub use repo::open_repo;
pub use branches::{get_current_branch, get_local_branches, get_remote_branches};
pub use commits::get_recent_commits;
pub use status::get_working_tree_status;
pub use stash::get_stashes;
pub use diff::{
    get_staged_diff, get_staged_files, get_unstaged_diff, get_unstaged_files, has_staged_changes,
    stage_all,
};
pub use commit_ops::{create_commit, get_author_info};

use anyhow::Result;
use git2::Repository;

use crate::models::RepoSummary;

pub fn gather_summary(repo: &mut Repository, commit_limit: usize) -> Result<RepoSummary> {
    let current_branch = get_current_branch(repo)?;
    let status = get_working_tree_status(repo)?;
    let recent_commits = get_recent_commits(repo, commit_limit)?;
    let local_branches = get_local_branches(repo)?;
    let remote_branches = get_remote_branches(repo)?;
    let stashes = get_stashes(repo)?;

    Ok(RepoSummary {
        current_branch,
        status,
        recent_commits,
        local_branches,
        remote_branches,
        stashes,
        graph: None,
    })
}
