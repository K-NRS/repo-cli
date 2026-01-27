use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

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
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_ai: None,
            show_github_stats: true,
            commit_style: None,
            auto_fetch: false,
            message_box_style: MessageBoxStyle::default(),
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
