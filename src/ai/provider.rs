use anyhow::{bail, Result};
use std::process::Command;

use super::{claude, codex, gemini};

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

/// Generate commit message using the specified provider
pub fn generate_commit_message(provider: AiProvider, diff: &str, style: Option<&str>) -> Result<String> {
    if diff.is_empty() {
        bail!("No staged changes to generate commit message for");
    }

    match provider {
        AiProvider::Claude => claude::generate(diff, style),
        AiProvider::Codex => codex::generate(diff, style),
        AiProvider::Gemini => gemini::generate(diff, style),
    }
}
