pub mod git;
pub mod models;
pub mod render;

pub use git::gather_summary;
pub use models::RepoSummary;
pub use render::{render_static, run_tui};
