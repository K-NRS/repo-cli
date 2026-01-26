use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

const BASE_PROMPT: &str = r#"Generate a git commit message for the following diff.
Follow conventional commit format: type(scope): description
Types: feat, fix, docs, style, refactor, test, chore
Only output the commit message, nothing else."#;

pub fn generate(diff: &str, style: Option<&str>) -> Result<String> {
    let style_instruction = match style {
        Some(s) => format!("\nStyle: {}", s),
        None => "\nKeep the first line under 72 characters. Be concise.".to_string(),
    };
    let input = format!("{}{}\n\n```diff\n{}\n```", BASE_PROMPT, style_instruction, diff);

    let mut child = Command::new("codex")
        .arg("-q")
        .arg("-c")
        .arg("history.persistence=none")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn codex CLI")?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .context("Failed to write to codex stdin")?;
    }

    let output = child.wait_with_output().context("Failed to wait for codex")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Codex failed: {}", stderr);
    }

    let message = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    Ok(message)
}
