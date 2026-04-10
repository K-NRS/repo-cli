pub mod filter;
pub mod groups;
pub mod render;
pub mod scan;
pub mod tui;

use chrono::{DateTime, Local};
use std::path::PathBuf;

use crate::models::{CommitInfo, UpstreamInfo, WorkingTreeStatus};

#[derive(Debug, Clone)]
pub enum WorkspaceSource {
    Path(PathBuf),
    Group { alias: String, root: PathBuf },
}

impl WorkspaceSource {
    pub fn display(&self) -> String {
        match self {
            WorkspaceSource::Path(p) => p.display().to_string(),
            WorkspaceSource::Group { alias, .. } => format!("@{}", alias),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceSummary {
    pub source: WorkspaceSource,
    pub repos: Vec<RepoSnapshot>,
    pub errors: Vec<(PathBuf, String)>,
}

#[derive(Debug, Clone)]
pub struct RepoSnapshot {
    pub path: PathBuf,
    pub name: String,
    pub branch: String,
    pub status: WorkingTreeStatus,
    pub upstream: Option<UpstreamInfo>,
    pub recent_commits: Vec<CommitInfo>,
    pub last_activity: Option<DateTime<Local>>,
    pub stale: bool,
}

impl RepoSnapshot {
    pub fn is_dirty(&self) -> bool {
        !self.status.is_clean()
    }

    pub fn ahead(&self) -> usize {
        self.upstream.as_ref().map(|u| u.ahead).unwrap_or(0)
    }

    pub fn behind(&self) -> usize {
        self.upstream.as_ref().map(|u| u.behind).unwrap_or(0)
    }
}
