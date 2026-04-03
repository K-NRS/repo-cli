use anyhow::{Context, Result};
use git2::{DiffOptions, IndexAddOption, Repository};

/// Get the staged diff as a string for AI consumption
pub fn get_staged_diff(repo: &Repository) -> Result<String> {
    let head = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index().context("Failed to get index")?;

    let mut opts = DiffOptions::new();
    opts.include_untracked(false);

    let diff = repo
        .diff_tree_to_index(head.as_ref(), Some(&index), Some(&mut opts))
        .context("Failed to create diff")?;

    let stats = diff.stats().context("Failed to get diff stats")?;
    if stats.files_changed() == 0 {
        return Ok(String::new());
    }

    let mut diff_text = String::new();

    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            'H' => "", // file header
            'F' => "", // file header
            'B' => "", // binary
            _ => "",
        };

        if !prefix.is_empty() {
            diff_text.push_str(prefix);
        }

        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }

        true
    })
    .context("Failed to print diff")?;

    Ok(diff_text)
}

/// Get list of staged files
pub fn get_staged_files(repo: &Repository) -> Result<Vec<String>> {
    let head = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index().context("Failed to get index")?;

    let diff = repo
        .diff_tree_to_index(head.as_ref(), Some(&index), None)
        .context("Failed to create diff")?;

    let mut files = Vec::new();
    for delta in diff.deltas() {
        if let Some(path) = delta.new_file().path() {
            files.push(path.to_string_lossy().to_string());
        }
    }

    Ok(files)
}

/// Check if there are any staged changes
pub fn has_staged_changes(repo: &Repository) -> Result<bool> {
    let head = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index().context("Failed to get index")?;

    let diff = repo
        .diff_tree_to_index(head.as_ref(), Some(&index), None)
        .context("Failed to create diff")?;

    let stats = diff.stats().context("Failed to get diff stats")?;
    Ok(stats.files_changed() > 0)
}

/// Stage all changes (modified + untracked), skipping worktree paths
pub fn stage_all(repo: &Repository) -> Result<()> {
    let mut index = repo.index().context("Failed to get index")?;
    index
        .add_all(
            ["*"].iter(),
            IndexAddOption::DEFAULT,
            Some(&mut |path: &std::path::Path, _spec: &[u8]| {
                if should_skip_path(path) {
                    1 // skip
                } else {
                    0 // add
                }
            }),
        )
        .context("Failed to add files to index")?;
    write_index(&mut index)?;
    Ok(())
}

/// Stage specific files by path
pub fn stage_files(repo: &Repository, paths: &[String]) -> Result<()> {
    let mut index = repo.index().context("Failed to get index")?;
    for path in paths {
        let p = std::path::Path::new(path);
        if p.exists() {
            index
                .add_path(p)
                .with_context(|| format!("Failed to stage: {}", path))?;
        } else {
            // Deleted file: stage the removal
            index
                .remove_path(p)
                .with_context(|| format!("Failed to stage removal: {}", path))?;
        }
    }
    write_index(&mut index)?;
    Ok(())
}

fn write_index(index: &mut git2::Index) -> Result<()> {
    index.write().map_err(|e| {
        if e.code() == git2::ErrorCode::Locked {
            anyhow::anyhow!(
                "Index is locked (likely a crashed git process)\n\n  Fix: rm -f .git/index.lock"
            )
        } else {
            anyhow::anyhow!("Failed to write index: {}", e)
        }
    })
}

/// Get list of unstaged files (modified + untracked)
pub fn get_unstaged_files(repo: &Repository) -> Result<Vec<(String, char)>> {
    use git2::StatusOptions;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut files = Vec::new();

    for entry in statuses.iter() {
        let status = entry.status();
        let path = entry.path().unwrap_or("").to_string();

        if should_skip_path(std::path::Path::new(&path)) {
            continue;
        }

        if status.is_wt_new() {
            files.push((path, '?'));
        } else if status.is_wt_modified() {
            files.push((path, 'M'));
        } else if status.is_wt_deleted() {
            files.push((path, 'D'));
        } else if status.is_wt_renamed() {
            files.push((path, 'R'));
        }
    }

    Ok(files)
}

/// Get the full diff for amend: parent of HEAD → current index
/// This shows all changes that will be in the amended commit
pub fn get_amend_diff(repo: &Repository) -> Result<String> {
    let head = repo.head().context("No HEAD")?;
    let head_commit = head.peel_to_commit().context("Failed to get HEAD commit")?;

    // Get parent tree (what we're comparing against)
    let parent_tree = if head_commit.parent_count() > 0 {
        Some(head_commit.parent(0)?.tree()?)
    } else {
        None // Initial commit - compare against empty tree
    };

    let index = repo.index().context("Failed to get index")?;

    let mut opts = DiffOptions::new();
    opts.include_untracked(false);

    let diff = repo
        .diff_tree_to_index(parent_tree.as_ref(), Some(&index), Some(&mut opts))
        .context("Failed to create diff")?;

    let stats = diff.stats().context("Failed to get diff stats")?;
    if stats.files_changed() == 0 {
        return Ok(String::new());
    }

    let mut diff_text = String::new();

    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            'H' => "",
            'F' => "",
            'B' => "",
            _ => "",
        };

        if !prefix.is_empty() {
            diff_text.push_str(prefix);
        }

        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }

        true
    })
    .context("Failed to print diff")?;

    Ok(diff_text)
}

/// Get the diff for a specific commit (parent tree -> commit tree)
pub fn get_commit_diff(repo: &Repository, oid: git2::Oid) -> Result<String> {
    let commit = repo.find_commit(oid).context("find commit")?;
    let commit_tree = commit.tree().context("commit tree")?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)
        .context("diff tree to tree")?;

    let mut diff_text = String::new();

    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            'H' => "",
            'F' => "",
            'B' => "",
            _ => "",
        };

        if !prefix.is_empty() {
            diff_text.push_str(prefix);
        }

        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }

        true
    })
    .context("print diff")?;

    Ok(diff_text)
}

/// Paths that should be excluded from staging (worktree directories, etc.)
fn should_skip_path(path: &std::path::Path) -> bool {
    path.starts_with(".claude/worktrees")
}

/// Get unstaged diff (working tree vs index)
pub fn get_unstaged_diff(repo: &Repository) -> Result<String> {
    let index = repo.index().context("Failed to get index")?;

    let mut opts = DiffOptions::new();
    opts.include_untracked(true);

    let diff = repo
        .diff_index_to_workdir(Some(&index), Some(&mut opts))
        .context("Failed to create diff")?;

    let mut diff_text = String::new();

    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            'H' => "",
            'F' => "",
            'B' => "",
            _ => "",
        };

        if !prefix.is_empty() {
            diff_text.push_str(prefix);
        }

        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }

        true
    })
    .context("Failed to print diff")?;

    Ok(diff_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn init_test_repo(dir: &Path) -> Repository {
        let repo = Repository::init(dir).unwrap();
        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            fs::write(dir.join("init.txt"), "init").unwrap();
            index.add_path(Path::new("init.txt")).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        {
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        repo
    }

    #[test]
    fn test_stage_all_skips_worktree_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_test_repo(tmp.path());

        let wt_dir = tmp.path().join(".claude/worktrees/agent-test123");
        fs::create_dir_all(&wt_dir).unwrap();
        fs::write(wt_dir.join("some_file.txt"), "worktree content").unwrap();

        fs::write(tmp.path().join("real_change.txt"), "real content").unwrap();

        stage_all(&repo).unwrap();

        let staged = get_staged_files(&repo).unwrap();
        assert!(staged.contains(&"real_change.txt".to_string()));
        assert!(!staged.iter().any(|f| f.contains(".claude/worktrees")));
    }

    #[test]
    fn test_get_unstaged_files_skips_worktree_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_test_repo(tmp.path());

        let wt_dir = tmp.path().join(".claude/worktrees/agent-abc");
        fs::create_dir_all(&wt_dir).unwrap();
        fs::write(wt_dir.join("file.rs"), "content").unwrap();

        fs::write(tmp.path().join("normal.txt"), "normal").unwrap();

        let files = get_unstaged_files(&repo).unwrap();
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();

        assert!(paths.contains(&"normal.txt"));
        assert!(!paths.iter().any(|p| p.contains(".claude/worktrees")));
    }

    #[test]
    fn test_should_skip_path() {
        assert!(should_skip_path(Path::new(".claude/worktrees/agent-123")));
        assert!(should_skip_path(Path::new(
            ".claude/worktrees/agent-123/file.txt"
        )));
        assert!(!should_skip_path(Path::new(".claude/settings.json")));
        assert!(!should_skip_path(Path::new("src/main.rs")));
    }
}
