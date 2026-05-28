use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

const BASE_PROMPT: &str = r#"You are a git commit message generator. Generate a git commit message for the following diff.
Follow conventional commit format: type(scope): description
Types: feat, fix, docs, style, refactor, test, chore
Output ONLY the raw commit message: no preamble, no markdown fences, no explanations, no alternatives, no questions, no follow-up.
Produce exactly ONE commit message for the entire diff; never propose splitting into multiple commits.
The full output must be usable verbatim as a commit message."#;

pub fn generate(diff: &str, style: Option<&str>, model: Option<&str>) -> Result<String> {
    let style_instruction = match style {
        Some(s) => format!("\nStyle: {}", s),
        None => "\nKeep the first line under 72 characters. Be concise.".to_string(),
    };
    let input = format!("{}{}\n\n```diff\n{}\n```", BASE_PROMPT, style_instruction, diff);

    let mut cmd = Command::new("gemini");
    if let Some(m) = model {
        cmd.arg("--model").arg(m);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn gemini CLI")?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .context("Failed to write to gemini stdin")?;
    }

    let output = child.wait_with_output().context("Failed to wait for gemini")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Gemini failed: {}", stderr);
    }

    let message = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    Ok(message)
}
