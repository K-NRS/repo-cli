use anyhow::{bail, Result};
use std::process::Command;

use super::{claude, codex, gemini};

/// Max characters to send to AI providers (~8K lines ≈ safe for all providers)
const MAX_DIFF_CHARS: usize = 40_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AiProvider {
    Claude,
    Codex,
    Gemini,
}

impl AiProvider {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }
}

/// Detect available AI CLI in priority order: claude → codex → gemini
pub fn detect_provider() -> Option<AiProvider> {
    let providers = [
        (AiProvider::Claude, "claude"),
        (AiProvider::Codex, "codex"),
        (AiProvider::Gemini, "gemini"),
    ];

    for (provider, cmd) in providers {
        if is_command_available(cmd) {
            return Some(provider);
        }
    }

    None
}

fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Truncate diff to fit AI provider limits, preserving file summary context
fn truncate_diff(diff: &str) -> String {
    if diff.len() <= MAX_DIFF_CHARS {
        return diff.to_string();
    }

    // Build file summary from diff headers
    let files: Vec<&str> = diff
        .lines()
        .filter(|l| l.starts_with("diff --git") || l.starts_with("+++ ") || l.starts_with("--- "))
        .collect();

    let summary = if files.is_empty() {
        String::new()
    } else {
        format!(
            "[FILES CHANGED]\n{}\n\n",
            files.join("\n")
        )
    };

    // Truncate the detailed diff
    let budget = MAX_DIFF_CHARS.saturating_sub(summary.len() + 80);
    let truncated: String = diff.chars().take(budget).collect();

    // Cut at last complete line
    let cut_point = truncated.rfind('\n').unwrap_or(truncated.len());

    format!(
        "{}{}\n\n[TRUNCATED — diff was {} chars, showing first {}]",
        summary,
        &truncated[..cut_point],
        diff.len(),
        cut_point
    )
}

/// Strip markdown code blocks (```) from AI output
pub fn strip_code_blocks(text: &str) -> String {
    let trimmed = text.trim();

    // Check if wrapped in code blocks
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() < 3 {
            return trimmed.to_string();
        }

        // Skip first line (``` or ```language) and last line (```)
        lines[1..lines.len() - 1]
            .join("\n")
            .trim()
            .to_string()
    } else {
        trimmed.to_string()
    }
}

/// Generate commit message using the specified provider
pub fn generate_commit_message(provider: AiProvider, diff: &str, style: Option<&str>) -> Result<String> {
    if diff.is_empty() {
        bail!("No staged changes to generate commit message for");
    }

    let diff = truncate_diff(diff);

    let message = match provider {
        AiProvider::Claude => claude::generate(&diff, style),
        AiProvider::Codex => codex::generate(&diff, style),
        AiProvider::Gemini => gemini::generate(&diff, style),
    }?;

    Ok(strip_code_blocks(&message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_code_blocks() {
        // Plain text - no changes
        assert_eq!(
            strip_code_blocks("fix(auth): handle null session"),
            "fix(auth): handle null session"
        );

        // Code block without language
        assert_eq!(
            strip_code_blocks("```\nfix(auth): handle null session\n```"),
            "fix(auth): handle null session"
        );

        // Code block with language
        assert_eq!(
            strip_code_blocks("```text\nfix(auth): handle null session\n```"),
            "fix(auth): handle null session"
        );

        // Multiline commit message
        assert_eq!(
            strip_code_blocks("```\nfix(auth): handle null session\n\nPrevents crash when user logs out\n```"),
            "fix(auth): handle null session\n\nPrevents crash when user logs out"
        );

        // Incomplete code block (only opening)
        assert_eq!(
            strip_code_blocks("```\nfix(auth): handle null session"),
            "```\nfix(auth): handle null session"
        );

        // Extra whitespace
        assert_eq!(
            strip_code_blocks("  ```\nfix(auth): handle null session\n```  "),
            "fix(auth): handle null session"
        );
    }
}
