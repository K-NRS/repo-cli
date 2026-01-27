use anyhow::{Context, Result};
use git2::{Oid, Repository};

#[derive(Debug, Clone)]
pub struct Hunk {
    pub file_path: String,
    pub header: String,
    pub lines: Vec<DiffLine>,
    pub old_start: u32,
    pub new_start: u32,
}

#[derive(Debug, Clone)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

impl Hunk {
    pub fn summary(&self) -> String {
        let added = self.lines.iter().filter(|l| matches!(l, DiffLine::Added(_))).count();
        let removed = self.lines.iter().filter(|l| matches!(l, DiffLine::Removed(_))).count();
        format!("{} +{} -{}", self.file_path, added, removed)
    }
}

pub fn get_commit_hunks(repo: &Repository, commit_oid: Oid) -> Result<Vec<Hunk>> {
    let commit = repo.find_commit(commit_oid).context("find commit")?;
    let commit_tree = commit.tree().context("commit tree")?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)
        .context("diff tree to tree")?;

    // Parse hunks by iterating over deltas/patches (avoids borrow issues with foreach)
    let mut hunks: Vec<Hunk> = Vec::new();

    for delta_idx in 0..diff.deltas().len() {
        let patch = git2::Patch::from_diff(&diff, delta_idx)
            .context("get patch")?;

        if let Some(patch) = patch {
            let file_path = diff
                .get_delta(delta_idx)
                .and_then(|d| d.new_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            for hunk_idx in 0..patch.num_hunks() {
                let (hunk_header, _) = patch.hunk(hunk_idx).context("get hunk")?;

                let mut lines = Vec::new();
                let num_lines = patch.num_lines_in_hunk(hunk_idx).context("num lines")?;

                for line_idx in 0..num_lines {
                    let line = patch.line_in_hunk(hunk_idx, line_idx).context("get line")?;
                    let content = std::str::from_utf8(line.content())
                        .unwrap_or("")
                        .to_string();
                    match line.origin() {
                        '+' => lines.push(DiffLine::Added(content)),
                        '-' => lines.push(DiffLine::Removed(content)),
                        ' ' => lines.push(DiffLine::Context(content)),
                        _ => {}
                    }
                }

                hunks.push(Hunk {
                    file_path: file_path.clone(),
                    header: std::str::from_utf8(hunk_header.header())
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                    lines,
                    old_start: hunk_header.old_start(),
                    new_start: hunk_header.new_start(),
                });
            }
        }
    }

    Ok(hunks)
}

pub fn generate_patch_for_hunks(hunks: &[Hunk], selected: &[usize]) -> String {
    let mut patch = String::new();
    let mut current_file: Option<&str> = None;

    for &idx in selected {
        let hunk = &hunks[idx];

        if current_file != Some(&hunk.file_path) {
            // file header
            patch.push_str(&format!("--- a/{}\n", hunk.file_path));
            patch.push_str(&format!("+++ b/{}\n", hunk.file_path));
            current_file = Some(&hunk.file_path);
        }

        // hunk header
        patch.push_str(&format!("{}\n", hunk.header));

        // lines
        for line in &hunk.lines {
            match line {
                DiffLine::Context(s) => {
                    patch.push(' ');
                    patch.push_str(s);
                }
                DiffLine::Added(s) => {
                    patch.push('+');
                    patch.push_str(s);
                }
                DiffLine::Removed(s) => {
                    patch.push('-');
                    patch.push_str(s);
                }
            }
            if !patch.ends_with('\n') {
                patch.push('\n');
            }
        }
    }

    patch
}
