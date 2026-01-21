use anyhow::Result;
use git2::Repository;

use crate::models::StashInfo;

pub fn get_stashes(repo: &mut Repository) -> Result<Vec<StashInfo>> {
    let mut stashes = Vec::new();

    repo.stash_foreach(|index, message, _oid| {
        stashes.push(StashInfo {
            index,
            message: message.to_string(),
        });
        true // continue iteration
    })?;

    Ok(stashes)
}
