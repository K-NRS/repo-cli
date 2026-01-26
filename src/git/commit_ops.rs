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

/// Amend the last commit with new staged changes and/or message
pub fn amend_commit(repo: &Repository, message: &str) -> Result<git2::Oid> {
    let sig = repo
        .signature()
        .context("Failed to get default signature. Configure git user.name and user.email")?;

    let head = repo.head().context("No HEAD commit to amend")?;
    let parent = head.peel_to_commit().context("Failed to get HEAD commit")?;

    let mut index = repo.index().context("Failed to get index")?;
    let tree_id = index.write_tree().context("Failed to write tree")?;
    let tree = repo.find_tree(tree_id).context("Failed to find tree")?;

    let oid = parent
        .amend(Some("HEAD"), Some(&sig), Some(&sig), None, Some(message), Some(&tree))
        .context("Failed to amend commit")?;

    Ok(oid)
}

/// Get the message from the last commit
pub fn get_last_commit_message(repo: &Repository) -> Result<String> {
    let head = repo.head().context("No HEAD commit")?;
    let commit = head.peel_to_commit().context("Failed to get HEAD commit")?;
    let message = commit.message().unwrap_or("").to_string();
    Ok(message.trim().to_string())
}

/// Get the current user signature for display purposes
pub fn get_author_info(repo: &Repository) -> Result<(String, String)> {
    let sig = repo.signature().context("Failed to get signature")?;
    let name = sig.name().unwrap_or("Unknown").to_string();
    let email = sig.email().unwrap_or("").to_string();
    Ok((name, email))
}
