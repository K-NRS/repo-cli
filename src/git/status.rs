use anyhow::Result;
use git2::{Repository, StatusOptions};

use crate::models::WorkingTreeStatus;

pub fn get_working_tree_status(repo: &Repository) -> Result<WorkingTreeStatus> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);

    let statuses = repo.statuses(Some(&mut opts))?;

    let mut staged = 0;
    let mut modified = 0;
    let mut untracked = 0;
    let mut conflicted = 0;

    for entry in statuses.iter() {
        let status = entry.status();

        if status.is_conflicted() {
            conflicted += 1;
        } else if status.is_wt_new() {
            untracked += 1;
        } else {
            // Count staged changes
            if status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_renamed()
                || status.is_index_typechange()
            {
                staged += 1;
            }

            // Count working tree changes
            if status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_renamed()
                || status.is_wt_typechange()
            {
                modified += 1;
            }
        }
    }

    Ok(WorkingTreeStatus {
        staged,
        modified,
        untracked,
        conflicted,
    })
}
