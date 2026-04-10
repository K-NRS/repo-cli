use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub alias: String,
    /// Directory to scan for git repos (optional if `pinned` covers everything)
    #[serde(default)]
    pub scan_root: Option<String>,
    /// Max directory depth when scanning (default: 3)
    #[serde(default = "default_depth")]
    pub max_depth: usize,
    /// Glob patterns to exclude from scan results
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Explicit additional repo paths (beyond scan results)
    #[serde(default)]
    pub pinned: Vec<String>,
    /// Explicit repo paths to exclude from scan (by absolute path match)
    #[serde(default)]
    pub unpinned: Vec<String>,
}

fn default_depth() -> usize {
    3
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GroupsFile {
    #[serde(rename = "group", default)]
    pub groups: Vec<Group>,
}

impl GroupsFile {
    pub fn path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("repo")
            .join("groups.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let file: GroupsFile = toml::from_str(&content)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(file)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .context("serializing groups.toml")?;
        fs::write(&path, content)
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    pub fn find(&self, alias: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.alias == alias)
    }

    pub fn upsert(&mut self, group: Group) {
        if let Some(existing) = self.groups.iter_mut().find(|g| g.alias == group.alias) {
            *existing = group;
        } else {
            self.groups.push(group);
        }
    }

    pub fn remove(&mut self, alias: &str) -> bool {
        let len = self.groups.len();
        self.groups.retain(|g| g.alias != alias);
        self.groups.len() != len
    }
}

/// Expand leading ~ and env vars in a path string
pub fn expand_path(s: &str) -> PathBuf {
    let s = s.trim();
    if let Some(stripped) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    } else if s == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(s)
}

/// Contract a home path back to ~/ form for display/storage
pub fn contract_path(p: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(stripped) = p.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    p.display().to_string()
}
