use anyhow::Result;
use clap::Parser;

use repo::git::{gather_summary, open_repo};
use repo::render::{render_static, run_tui};

#[derive(Parser, Debug)]
#[command(name = "repo")]
#[command(about = "A visual git repository summary tool", long_about = None)]
struct Args {
    /// Run in interactive TUI mode
    #[arg(short, long)]
    interactive: bool,

    /// Show full ASCII branch graph
    #[arg(long)]
    graph: bool,

    /// Disable colored output
    #[arg(long)]
    no_color: bool,

    /// Number of recent commits to show (default: 5)
    #[arg(short = 'n', long, default_value = "5")]
    commits: usize,

    /// Path to git repository (defaults to current directory)
    #[arg(value_name = "PATH")]
    path: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut repo = match &args.path {
        Some(p) => open_repo(Some(std::path::Path::new(p)))?,
        None => open_repo(None)?,
    };

    let summary = gather_summary(&mut repo, args.commits)?;

    if args.interactive {
        run_tui(summary)?;
    } else {
        render_static(&summary, args.graph, !args.no_color);
    }

    Ok(())
}
