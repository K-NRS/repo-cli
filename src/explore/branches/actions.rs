use anyhow::Result;
use git2::Repository;
use std::process::Command;

use crate::git::get_working_tree_status;

pub enum ActionResult {
    Success(String),
    Error(String),
    NeedsStash,
}

pub fn checkout_branch(
    repo_path: &str,
    branch_name: &str,
    repo: &Repository,
) -> Result<ActionResult> {
    let status = get_working_tree_status(repo)?;
    if !status.is_clean() {
        return Ok(ActionResult::NeedsStash);
    }

    let output = Command::new("git")
        .args(["-C", repo_path, "checkout", branch_name])
        .output()?;

    if output.status.success() {
        Ok(ActionResult::Success(format!("Checked out {}", branch_name)))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(ActionResult::Error(format!(
            "Checkout failed: {}",
            stderr.trim()
        )))
    }
}

pub fn delete_branch(repo_path: &str, branch_name: &str, force: bool) -> Result<ActionResult> {
    let flag = if force { "-D" } else { "-d" };
    let output = Command::new("git")
        .args(["-C", repo_path, "branch", flag, branch_name])
        .output()?;

    if output.status.success() {
        Ok(ActionResult::Success(format!("Deleted {}", branch_name)))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(ActionResult::Error(format!(
            "Delete failed: {}",
            stderr.trim()
        )))
    }
}

pub fn create_branch(repo_path: &str, new_name: &str, from_ref: &str) -> Result<ActionResult> {
    let output = Command::new("git")
        .args(["-C", repo_path, "branch", new_name, from_ref])
        .output()?;

    if output.status.success() {
        Ok(ActionResult::Success(format!(
            "Created {} from {}",
            new_name, from_ref
        )))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(ActionResult::Error(format!(
            "Create failed: {}",
            stderr.trim()
        )))
    }
}

pub fn merge_branch(
    repo_path: &str,
    branch_name: &str,
    repo: &Repository,
) -> Result<ActionResult> {
    let status = get_working_tree_status(repo)?;
    if !status.is_clean() {
        return Ok(ActionResult::NeedsStash);
    }

    let output = Command::new("git")
        .args(["-C", repo_path, "merge", branch_name])
        .output()?;

    if output.status.success() {
        Ok(ActionResult::Success(format!("Merged {}", branch_name)))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(ActionResult::Error(format!(
            "Merge failed: {}",
            stderr.trim()
        )))
    }
}

pub fn rebase_onto(
    repo_path: &str,
    branch_name: &str,
    repo: &Repository,
) -> Result<ActionResult> {
    let status = get_working_tree_status(repo)?;
    if !status.is_clean() {
        return Ok(ActionResult::NeedsStash);
    }

    let output = Command::new("git")
        .args(["-C", repo_path, "rebase", branch_name])
        .output()?;

    if output.status.success() {
        Ok(ActionResult::Success(format!(
            "Rebased onto {}",
            branch_name
        )))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = Command::new("git")
            .args(["-C", repo_path, "rebase", "--abort"])
            .output();
        Ok(ActionResult::Error(format!(
            "Rebase failed: {}",
            stderr.trim()
        )))
    }
}

pub fn cherry_pick(
    repo_path: &str,
    commit_hash: &str,
    repo: &Repository,
) -> Result<ActionResult> {
    let status = get_working_tree_status(repo)?;
    if !status.is_clean() {
        return Ok(ActionResult::NeedsStash);
    }

    let output = Command::new("git")
        .args(["-C", repo_path, "cherry-pick", commit_hash])
        .output()?;

    if output.status.success() {
        Ok(ActionResult::Success(format!(
            "Cherry-picked {}",
            &commit_hash[..7.min(commit_hash.len())]
        )))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = Command::new("git")
            .args(["-C", repo_path, "cherry-pick", "--abort"])
            .output();
        Ok(ActionResult::Error(format!(
            "Cherry-pick failed: {}",
            stderr.trim()
        )))
    }
}

pub fn revert_commit(
    repo_path: &str,
    commit_hash: &str,
    repo: &Repository,
) -> Result<ActionResult> {
    let status = get_working_tree_status(repo)?;
    if !status.is_clean() {
        return Ok(ActionResult::NeedsStash);
    }

    let output = Command::new("git")
        .args(["-C", repo_path, "revert", "--no-edit", commit_hash])
        .output()?;

    if output.status.success() {
        Ok(ActionResult::Success(format!(
            "Reverted {}",
            &commit_hash[..7.min(commit_hash.len())]
        )))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = Command::new("git")
            .args(["-C", repo_path, "revert", "--abort"])
            .output();
        Ok(ActionResult::Error(format!(
            "Revert failed: {}",
            stderr.trim()
        )))
    }
}
