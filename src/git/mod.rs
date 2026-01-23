mod repo;
mod branches;
mod commits;
mod status;
mod stash;
mod diff;
mod commit_ops;
mod github;
mod fetch;

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
pub use github::{get_github_stats, get_stargazers, get_forks, GithubStats, Stargazer, Fork};
pub use fetch::{fetch_all_remotes, print_fetch_warnings};

use anyhow::Result;
use git2::Repository;

use crate::config::Config;
use crate::models::RepoSummary;

pub fn gather_summary(repo: &mut Repository, commit_limit: usize) -> Result<RepoSummary> {
    let current_branch = get_current_branch(repo)?;
    let status = get_working_tree_status(repo)?;
    let recent_commits = get_recent_commits(repo, commit_limit)?;
    let local_branches = get_local_branches(repo)?;
    let remote_branches = get_remote_branches(repo)?;
    let stashes = get_stashes(repo)?;

    let config = Config::load().unwrap_or_default();
    let github_stats = if config.show_github_stats {
        get_github_stats(repo)
    } else {
        None
    };

    Ok(RepoSummary {
        current_branch,
        status,
        recent_commits,
        local_branches,
        remote_branches,
        stashes,
        graph: None,
        github_stars: github_stats.as_ref().map(|s| s.stars),
        github_forks: github_stats.as_ref().map(|s| s.forks),
    })
}
