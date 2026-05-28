use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

// Sent via --system-prompt so it replaces Claude Code's default agent prompt.
// Without this, `claude -p` runs the full agent: it loads the user's output style
// and CLAUDE.md rules (e.g. "split mixed-concern commits", "ask options") and
// answers conversationally instead of emitting a bare commit message.
const SYSTEM_PROMPT: &str = r#"You are a git commit message generator. Output ONLY the raw commit message in conventional commit format: type(scope): description
Allowed types: feat, fix, docs, style, refactor, test, chore.
Rules:
- Output the commit message and nothing else: no preamble, no markdown fences, no explanations, no alternatives, no questions, no follow-up.
- Produce exactly ONE commit message for the entire diff. Never propose splitting into multiple commits.
- The full output must be usable verbatim as a commit message."#;

pub fn generate(diff: &str, style: Option<&str>, model: Option<&str>) -> Result<String> {
    let style_instruction = match style {
        Some(s) => format!("Style: {}", s),
        None => "Keep the first line under 72 characters. Be concise.".to_string(),
    };
    let input = format!(
        "{}\n\nGenerate the commit message for this diff:\n\n```diff\n{}\n```",
        style_instruction, diff
    );

    let mut cmd = Command::new(super::path::resolve("claude"));
    // --setting-sources "" skips user/project/local settings (output style + CLAUDE.md);
    // --system-prompt replaces the conversational default agent prompt.
    cmd.arg("-p")
        .arg("--no-session-persistence")
        .arg("--setting-sources")
        .arg("")
        .arg("--system-prompt")
        .arg(SYSTEM_PROMPT);
    if let Some(m) = model {
        cmd.arg("--model").arg(m);
    }
    let mut child = cmd
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
