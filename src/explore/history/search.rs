use anyhow::Result;
use git2::Oid;
use std::process::Command;

pub fn search_diff_content(repo_path: &str, term: &str, limit: usize) -> Result<Vec<Oid>> {
    let output = Command::new("git")
        .args([
            "-C",
            repo_path,
            "log",
            "--format=%H",
            "-S",
            term,
            &format!("-{}", limit),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git log -S failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| Oid::from_str(line.trim()).ok())
        .collect())
}

pub fn search_by_path(repo_path: &str, path: &str, limit: usize) -> Result<Vec<Oid>> {
    let output = Command::new("git")
        .args([
            "-C",
            repo_path,
            "log",
            "--format=%H",
            &format!("-{}", limit),
            "--",
            path,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git log -- path failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| Oid::from_str(line.trim()).ok())
        .collect())
}

pub fn get_file_history(repo_path: &str, file_path: &str, limit: usize) -> Result<Vec<Oid>> {
    let output = Command::new("git")
        .args([
            "-C",
            repo_path,
            "log",
            "--follow",
            "--format=%H",
            &format!("-{}", limit),
            "--",
            file_path,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git log --follow failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| Oid::from_str(line.trim()).ok())
        .collect())
}
