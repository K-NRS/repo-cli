use anyhow::{bail, Result};
use std::process::Command;

use super::{claude, codex, gemini};

/// Max characters to send to AI providers
/// Claude CLI pipe mode has strict limits; keep conservative to avoid "Prompt is too long"
const MAX_DIFF_CHARS: usize = 8_000;

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

    // Parse diff into file chunks
    let file_chunks: Vec<&str> = split_diff_by_file(diff);
    let file_count = file_chunks.len();

    // For very large diffs (many files), use simple summary + truncated content
    if file_count > 50 || file_chunks.is_empty() {
        return truncate_simple(diff, file_count);
    }

    // Build file summary (only for reasonable number of files)
    let file_names: Vec<String> = file_chunks
        .iter()
        .filter_map(|chunk| {
            chunk.lines().find(|l| l.starts_with("diff --git"))
                .map(|l| extract_filename(l))
        })
        .collect();

    let summary = format!(
        "[{} FILES CHANGED]\n{}\n\n",
        file_count,
        file_names.join("\n")
    );

    // Budget for actual diff content
    let budget = MAX_DIFF_CHARS.saturating_sub(summary.len() + 100);

    if budget < 1000 {
        // Summary too large, fall back to simple truncation
        return truncate_simple(diff, file_count);
    }

    // Distribute budget across files (prioritize first files)
    let mut result = summary;
    let mut remaining = budget;
    let per_file_budget = budget / file_count.max(1);
    let mut truncated_files = 0;

    for (i, chunk) in file_chunks.iter().enumerate() {
        let weight = if i < 3 { 1.5 } else { 1.0 };
        let this_budget = ((per_file_budget as f64) * weight).min(remaining as f64) as usize;

        if this_budget < 200 {
            truncated_files += 1;
            continue;
        }

        if chunk.len() <= this_budget {
            result.push_str(chunk);
            result.push_str("\n");
            remaining = remaining.saturating_sub(chunk.len() + 1);
        } else {
            let truncated: String = chunk.chars().take(this_budget).collect();
            let cut_point = truncated.rfind('\n').unwrap_or(truncated.len());
            result.push_str(&truncated[..cut_point]);
            result.push_str("\n[...truncated...]\n");
            remaining = remaining.saturating_sub(cut_point + 20);
            truncated_files += 1;
        }
    }

    result.push_str(&format!(
        "\n[TRUNCATED — {} chars total, {} files shown{}]",
        diff.len(),
        file_count - truncated_files,
        if truncated_files > 0 { format!(", {} partially/skipped", truncated_files) } else { String::new() }
    ));

    result
}

/// Simple truncation for very large diffs with many files
fn truncate_simple(diff: &str, file_count: usize) -> String {
    // Reserve space for header and footer
    let content_budget = MAX_DIFF_CHARS.saturating_sub(200);

    let truncated: String = diff.chars().take(content_budget).collect();
    let cut_point = truncated.rfind('\n').unwrap_or(truncated.len());

    format!(
        "[{} FILES CHANGED — showing first {} chars of {} total]\n\n{}\n\n[TRUNCATED]",
        file_count,
        cut_point,
        diff.len(),
        &truncated[..cut_point]
    )
}

/// Split diff into chunks by file
fn split_diff_by_file(diff: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;

    for (i, _) in diff.match_indices("diff --git") {
        if i > start {
            let chunk = &diff[start..i];
            if !chunk.trim().is_empty() {
                chunks.push(chunk.trim());
            }
        }
        start = i;
    }

    // Last chunk
    if start < diff.len() {
        let chunk = &diff[start..];
        if !chunk.trim().is_empty() {
            chunks.push(chunk.trim());
        }
    }

    chunks
}

/// Extract filename from diff --git line
fn extract_filename(line: &str) -> String {
    // "diff --git a/path/to/file b/path/to/file"
    line.strip_prefix("diff --git ")
        .and_then(|s| s.split_whitespace().next())
        .map(|s| s.strip_prefix("a/").unwrap_or(s))
        .unwrap_or(line)
        .to_string()
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

    #[test]
    fn test_truncate_diff_small() {
        let small_diff = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n fn main() {\n+    println!(\"hello\");\n }";
        let result = truncate_diff(small_diff);
        // Small diff should pass through unchanged
        assert_eq!(result, small_diff);
    }

    #[test]
    fn test_truncate_diff_large() {
        // Create a large diff (> MAX_DIFF_CHARS)
        let file1 = format!(
            "diff --git a/src/foo.rs b/src/foo.rs\n--- a/src/foo.rs\n+++ b/src/foo.rs\n{}",
            "+line\n".repeat(2000)
        );
        let file2 = format!(
            "diff --git a/src/bar.rs b/src/bar.rs\n--- a/src/bar.rs\n+++ b/src/bar.rs\n{}",
            "+another line\n".repeat(2000)
        );
        let large_diff = format!("{}\n{}", file1, file2);

        let result = truncate_diff(&large_diff);

        // Should be truncated
        assert!(result.len() <= MAX_DIFF_CHARS + 200); // some margin for summary
        // Should contain file summary
        assert!(result.contains("[2 FILES CHANGED]"));
        assert!(result.contains("src/foo.rs"));
        assert!(result.contains("src/bar.rs"));
        // Should indicate truncation
        assert!(result.contains("TRUNCATED"));
    }

    #[test]
    fn test_extract_filename() {
        assert_eq!(
            extract_filename("diff --git a/src/main.rs b/src/main.rs"),
            "src/main.rs"
        );
        assert_eq!(
            extract_filename("diff --git a/path/to/file.txt b/path/to/file.txt"),
            "path/to/file.txt"
        );
    }

    #[test]
    fn test_split_diff_by_file() {
        let diff = "diff --git a/foo.rs b/foo.rs\n+foo\ndiff --git a/bar.rs b/bar.rs\n+bar";
        let chunks = split_diff_by_file(diff);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].starts_with("diff --git a/foo.rs"));
        assert!(chunks[1].starts_with("diff --git a/bar.rs"));
    }
}
