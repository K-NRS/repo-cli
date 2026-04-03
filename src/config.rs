use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageBoxStyle {
    Box,
    DoubleLine,
    TitleBox,
    Gutter,
}

impl Default for MessageBoxStyle {
    fn default() -> Self {
        Self::Box
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_ai: Option<String>,

    #[serde(default = "default_true")]
    pub show_github_stats: bool,

    #[serde(default)]
    pub commit_style: Option<String>,

    /// Automatically fetch from remotes before showing summary
    #[serde(default)]
    pub auto_fetch: bool,

    /// Style for commit message display: box, double_line, title_box, gutter
    #[serde(default)]
    pub message_box_style: MessageBoxStyle,

    /// Default AI provider for explore features
    #[serde(default)]
    pub ai_provider: Option<String>,

    /// Default AI model for explore features
    #[serde(default)]
    pub ai_model: Option<String>,

    /// Days before a branch is considered stale (default: 30)
    #[serde(default = "default_stale_days")]
    pub stale_branch_days: u64,

    /// Glob patterns for files to never stage/commit (global)
    #[serde(default)]
    pub ignore_files: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_stale_days() -> u64 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_ai: None,
            show_github_stats: true,
            commit_style: None,
            auto_fetch: false,
            message_box_style: MessageBoxStyle::default(),
            ai_provider: None,
            ai_model: None,
            stale_branch_days: 30,
            ignore_files: Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("repo")
            .join("config.toml")
    }
}

/// Load per-repo `.repoignore` patterns (gitignore-style: one glob per line, # comments)
pub fn load_repo_ignore(repo_root: &Path) -> Vec<String> {
    let path = repo_root.join(".repoignore");
    match fs::read_to_string(&path) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Build a GlobSet from merged config + .repoignore patterns.
/// Returns None if no patterns are configured.
pub fn build_ignore_set(config: &Config, repo_root: &Path) -> Option<GlobSet> {
    let mut patterns = config.ignore_files.clone();
    patterns.extend(load_repo_ignore(repo_root));

    if patterns.is_empty() {
        return None;
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in &patterns {
        // Support both "file.txt" and "**/file.txt" style patterns
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
        // Also add with **/ prefix for bare filenames without path separators
        if !pattern.contains('/') && !pattern.starts_with("**/") {
            if let Ok(glob) = Glob::new(&format!("**/{}", pattern)) {
                builder.add(glob);
            }
        }
    }

    builder.build().ok()
}
