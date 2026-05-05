use std::path::PathBuf;
use std::process::Command;

const CLAUDE_PATH_ENV: &str = "CLAUDE_CODE_PATH";

const FALLBACK_CLAUDE_PATHS: &[&str] = &[
    "~/.local/bin/claude",
    "~/.claude/local/claude",
];

pub fn resolve(name: &str) -> String {
    if name == "claude" {
        if let Ok(p) = std::env::var(CLAUDE_PATH_ENV) {
            if !p.trim().is_empty() {
                return p;
            }
        }
    }

    if which(name) {
        return name.to_string();
    }

    if name == "claude" {
        if let Some(p) = first_existing(FALLBACK_CLAUDE_PATHS) {
            return p;
        }
    }

    name.to_string()
}

pub fn is_available(name: &str) -> bool {
    if name == "claude" {
        if let Ok(p) = std::env::var(CLAUDE_PATH_ENV) {
            if !p.trim().is_empty() && PathBuf::from(&p).exists() {
                return true;
            }
        }
    }

    if which(name) {
        return true;
    }

    if name == "claude" && first_existing(FALLBACK_CLAUDE_PATHS).is_some() {
        return true;
    }

    false
}

fn which(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn first_existing(candidates: &[&str]) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    for c in candidates {
        let expanded = c.replacen('~', &home, 1);
        if PathBuf::from(&expanded).exists() {
            return Some(expanded);
        }
    }
    None
}
