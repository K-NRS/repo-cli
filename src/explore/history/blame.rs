use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct BlameLine {
    pub commit_hash: String,
    pub author: String,
    pub line_no: usize,
    pub content: String,
}

pub fn get_blame(repo_path: &str, file_path: &str, commit: &str) -> Result<Vec<BlameLine>> {
    let output = Command::new("git")
        .args([
            "-C",
            repo_path,
            "blame",
            "--porcelain",
            commit,
            "--",
            file_path,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git blame failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_porcelain_blame(&stdout)
}

fn parse_porcelain_blame(output: &str) -> Result<Vec<BlameLine>> {
    let mut lines = Vec::new();
    let mut current_hash = String::new();
    let mut current_author = String::new();
    let mut line_no = 0;

    for line in output.lines() {
        if line.starts_with('\t') {
            lines.push(BlameLine {
                commit_hash: current_hash.clone(),
                author: current_author.clone(),
                line_no,
                content: line[1..].to_string(),
            });
        } else if let Some(author) = line.strip_prefix("author ") {
            current_author = author.to_string();
        } else {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[0].len() == 40 {
                current_hash = parts[0][..7].to_string();
                line_no = parts[2].parse().unwrap_or(0);
            }
        }
    }

    Ok(lines)
}
