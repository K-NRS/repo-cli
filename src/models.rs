use chrono::{DateTime, Local};
use git2::Oid;

#[derive(Debug, Clone)]
pub struct RepoSummary {
    pub current_branch: BranchInfo,
    pub status: WorkingTreeStatus,
    pub recent_commits: Vec<CommitInfo>,
    pub local_branches: Vec<BranchInfo>,
    pub remote_branches: Vec<RemoteBranchInfo>,
    pub stashes: Vec<StashInfo>,
    pub graph: Option<BranchGraph>,
    pub github_stars: Option<u32>,
    pub github_forks: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub upstream: Option<UpstreamInfo>,
    pub tip_commit: Option<Oid>,
}

#[derive(Debug, Clone)]
pub struct UpstreamInfo {
    pub name: String,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Debug, Clone)]
pub struct WorkingTreeStatus {
    pub staged: usize,
    pub modified: usize,
    pub untracked: usize,
    pub conflicted: usize,
}

impl WorkingTreeStatus {
    pub fn is_clean(&self) -> bool {
        self.staged == 0 && self.modified == 0 && self.untracked == 0 && self.conflicted == 0
    }

    pub fn total_changes(&self) -> usize {
        self.staged + self.modified
    }
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub id: Oid,
    pub short_id: String,
    pub message: String,
    pub author: String,
    pub time: DateTime<Local>,
    pub parents: Vec<Oid>,
}

#[derive(Debug, Clone)]
pub struct RemoteBranchInfo {
    pub name: String,
    pub remote: String,
    pub short_name: String,
    pub last_commit_time: DateTime<Local>,
    pub last_commit_message: String,
    pub last_commit_author: String,
}

#[derive(Debug, Clone)]
pub struct StashInfo {
    pub index: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct BranchGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub commit_id: Oid,
    pub column: usize,
    pub branches: Vec<String>,
    pub is_merge: bool,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from_commit: Oid,
    pub to_commit: Oid,
    pub from_column: usize,
    pub to_column: usize,
}

pub fn format_relative_time(dt: &DateTime<Local>) -> String {
    let now = Local::now();
    let duration = now.signed_duration_since(*dt);

    if duration.num_minutes() < 1 {
        "now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}m", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h", duration.num_hours())
    } else if duration.num_days() < 7 {
        format!("{}d", duration.num_days())
    } else if duration.num_weeks() < 4 {
        format!("{}w", duration.num_weeks())
    } else {
        format!("{}mo", duration.num_days() / 30)
    }
}
