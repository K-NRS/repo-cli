mod repo;
mod branches;
mod commits;
mod status;
mod stash;

pub use repo::open_repo;
pub use branches::{get_current_branch, get_local_branches, get_remote_branches};
pub use commits::get_recent_commits;
pub use status::get_working_tree_status;
pub use stash::get_stashes;

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
