use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use walkdir::WalkDir;

use crate::config::Config;
use crate::git::{
    get_current_branch, get_recent_commits, get_working_tree_status, open_repo,
};
use crate::workspace::groups::{expand_path, Group};
use crate::workspace::{RepoSnapshot, WorkspaceSource, WorkspaceSummary};

const RECENT_COMMITS_PER_REPO: usize = 10;

pub struct ScanOptions {
    pub max_depth: usize,
    pub exclude: Vec<String>,
    pub extra_pinned: Vec<PathBuf>,
    pub unpinned: Vec<PathBuf>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            exclude: Vec::new(),
            extra_pinned: Vec::new(),
            unpinned: Vec::new(),
        }
    }
}

/// Discover all git repos under `root` up to `max_depth`, honoring excludes.
/// Detects both .git directories and .git files (worktrees).
pub fn discover_repos(root: &Path, opts: &ScanOptions) -> Vec<PathBuf> {
    let exclude_set = build_glob_set(&opts.exclude);
    let unpinned: Vec<PathBuf> = opts
        .unpinned
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect();

    let mut repos = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if root.exists() {
        let walker = WalkDir::new(root)
            .max_depth(opts.max_depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_hidden_or_noise(e.file_name().to_str().unwrap_or("")));

        for entry in walker.flatten() {
            let path = entry.path();
            if !is_git_marker(path) {
                continue;
            }
            // Parent of .git is the repo root
            let Some(repo_root) = path.parent() else {
                continue;
            };

            if let Some(ref set) = exclude_set {
                let rel = repo_root.strip_prefix(root).unwrap_or(repo_root);
                if set.is_match(rel) {
                    continue;
                }
            }

            let canonical = repo_root.canonicalize().unwrap_or_else(|_| repo_root.to_path_buf());
            if unpinned.contains(&canonical) {
                continue;
            }
            if seen.insert(canonical.clone()) {
                repos.push(canonical);
            }
        }
    }

    for pinned in &opts.extra_pinned {
        let canonical = pinned.canonicalize().unwrap_or_else(|_| pinned.clone());
        if seen.insert(canonical.clone()) {
            repos.push(canonical);
        }
    }

    repos.sort();
    repos
}

fn is_git_marker(path: &Path) -> bool {
    path.file_name().and_then(|s| s.to_str()) == Some(".git")
}

fn is_hidden_or_noise(name: &str) -> bool {
    // Allow traversing .git only as a leaf (matched above), skip other dotdirs and heavy junk
    matches!(
        name,
        "node_modules" | "target" | "dist" | "build" | ".next" | ".venv" | "venv"
    ) || (name.starts_with('.') && name != ".git" && name != "." && name != "..")
}

fn build_glob_set(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        if let Ok(g) = Glob::new(p) {
            builder.add(g);
        }
    }
    builder.build().ok()
}

/// Scan all repos under `root` in parallel, producing snapshots
pub fn scan_path(root: &Path, opts: &ScanOptions, config: &Config) -> Result<WorkspaceSummary> {
    let repo_paths = discover_repos(root, opts);
    let snapshots = load_snapshots_parallel(repo_paths, config);

    Ok(WorkspaceSummary {
        source: WorkspaceSource::Path(root.to_path_buf()),
        repos: snapshots.0,
        errors: snapshots.1,
    })
}

/// Scan repos defined by a group definition
pub fn scan_group(group: &Group, config: &Config) -> Result<WorkspaceSummary> {
    let opts = ScanOptions {
        max_depth: group.max_depth,
        exclude: group.exclude.clone(),
        extra_pinned: group.pinned.iter().map(|s| expand_path(s)).collect(),
        unpinned: group.unpinned.iter().map(|s| expand_path(s)).collect(),
    };

    let (root, repo_paths) = if let Some(scan_root) = &group.scan_root {
        let root = expand_path(scan_root);
        let paths = discover_repos(&root, &opts);
        (root, paths)
    } else {
        // Pinned-only group
        let root = PathBuf::from(".");
        let paths: Vec<PathBuf> = opts
            .extra_pinned
            .iter()
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
            .collect();
        (root, paths)
    };

    let (repos, errors) = load_snapshots_parallel(repo_paths, config);

    Ok(WorkspaceSummary {
        source: WorkspaceSource::Group {
            alias: group.alias.clone(),
            root,
        },
        repos,
        errors,
    })
}

fn load_snapshots_parallel(
    paths: Vec<PathBuf>,
    config: &Config,
) -> (Vec<RepoSnapshot>, Vec<(PathBuf, String)>) {
    if paths.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let stale_days = config.stale_branch_days;
    let worker_count = num_cpus::get().max(2).min(paths.len()).max(1);
    let (tx, rx) = mpsc::channel();
    let paths_shared = std::sync::Arc::new(std::sync::Mutex::new(paths.into_iter()));

    thread::scope(|s| {
        for _ in 0..worker_count {
            let tx = tx.clone();
            let paths_shared = std::sync::Arc::clone(&paths_shared);
            s.spawn(move || loop {
                let next = {
                    let mut guard = paths_shared.lock().unwrap();
                    guard.next()
                };
                let Some(path) = next else {
                    break;
                };
                let result = load_snapshot(&path, stale_days);
                let _ = tx.send((path, result));
            });
        }
    });
    drop(tx);

    let mut snapshots = Vec::new();
    let mut errors = Vec::new();
    for (path, result) in rx {
        match result {
            Ok(snap) => snapshots.push(snap),
            Err(e) => errors.push((path, e.to_string())),
        }
    }

    snapshots.sort_by(|a, b| {
        b.last_activity
            .cmp(&a.last_activity)
            .then_with(|| a.name.cmp(&b.name))
    });
    (snapshots, errors)
}

fn load_snapshot(path: &Path, stale_days: u64) -> Result<RepoSnapshot> {
    let repo = open_repo(Some(path))?;
    let branch_info = get_current_branch(&repo)?;
    let status = get_working_tree_status(&repo)?;
    let recent = get_recent_commits(&repo, RECENT_COMMITS_PER_REPO)?;

    let last_activity = recent.first().map(|c| c.time);
    let stale = last_activity
        .map(|t| {
            chrono::Local::now()
                .signed_duration_since(t)
                .num_days()
                .unsigned_abs()
                > stale_days
        })
        .unwrap_or(false);

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();

    Ok(RepoSnapshot {
        path: path.to_path_buf(),
        name,
        branch: branch_info.name,
        status,
        upstream: branch_info.upstream,
        recent_commits: recent,
        last_activity,
        stale,
    })
}
