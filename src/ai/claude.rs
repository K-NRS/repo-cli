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

    let mut child = Command::new("claude")
        .arg("-p")
        .arg("--no-session-persistence")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn claude CLI")?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .context("Failed to write to claude stdin")?;
    }

    let output = child.wait_with_output().context("Failed to wait for claude")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            if stdout.trim().is_empty() {
                format!("exit code {}", output.status.code().unwrap_or(-1))
            } else {
                stdout.trim().to_string()
            }
        } else {
            stderr.trim().to_string()
        };
        bail!("Claude failed: {}", detail);
    }

    let message = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    Ok(message)
}
