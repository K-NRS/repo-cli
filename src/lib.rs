pub mod git;
pub mod models;
pub mod render;
pub mod ai;
pub mod commit;

pub use git::gather_summary;
pub use models::RepoSummary;
pub use render::{render_static, run_tui};
pub use commit::run_commit_workflow;
