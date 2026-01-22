use anyhow::{bail, Context, Result};
use git2::Repository;

/// Create a commit with the given message
pub fn create_commit(repo: &Repository, message: &str) -> Result<git2::Oid> {
    let sig = repo
        .signature()
        .context("Failed to get default signature. Configure git user.name and user.email")?;

    let mut index = repo.index().context("Failed to get index")?;

    if index.is_empty() {
        bail!("Nothing to commit - index is empty");
    }

    let tree_id = index.write_tree().context("Failed to write tree")?;
    let tree = repo.find_tree(tree_id).context("Failed to find tree")?;

    let parent = match repo.head() {
        Ok(head) => {
            let commit = head.peel_to_commit().context("Failed to peel HEAD to commit")?;
            Some(commit)
        }
        Err(_) => None, // Initial commit
    };

    let parents: Vec<&git2::Commit> = parent.iter().collect();

    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .context("Failed to create commit")?;

    Ok(oid)
}

/// Get the current user signature for display purposes
pub fn get_author_info(repo: &Repository) -> Result<(String, String)> {
    let sig = repo.signature().context("Failed to get signature")?;
    let name = sig.name().unwrap_or("Unknown").to_string();
    let email = sig.email().unwrap_or("").to_string();
    Ok((name, email))
}
