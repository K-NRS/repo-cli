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

/// Stage all changes (modified + untracked)
pub fn stage_all(repo: &Repository) -> Result<()> {
    let mut index = repo.index().context("Failed to get index")?;
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .context("Failed to add files to index")?;
    index.write().context("Failed to write index")?;
    Ok(())
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
