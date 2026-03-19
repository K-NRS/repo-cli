pub mod tui;
pub mod layout;
pub mod overlay;
pub mod ai;
pub mod highlight;
pub mod history;
pub mod branches;

use anyhow::Result;
use git2::Repository;

use crate::config::Config;
use crate::models::RepoSummary;

pub fn run_explore(
    repo: Repository,
    summary: RepoSummary,
    tab: Option<String>,
    page_size: usize,
    config: &Config,
) -> Result<()> {
    tui::run_explore_tui(repo, summary, tab, page_size, config)
}
