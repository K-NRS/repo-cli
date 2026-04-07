use std::collections::HashMap;
use std::io::Write;
use std::process::Command;

use anyhow::Result;
use crate::ai::{AiProvider, detect_provider};
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct AiState {
    pub provider: Option<AiProvider>,
    pub model: Option<String>,
    pub available_providers: Vec<AiProvider>,
    pub cache: HashMap<AiCacheKey, String>,
    pub loading: bool,
    pub last_result: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum AiCacheKey {
    CommitSummary(String),
    CommitExplain(String),
    BranchSummary(String),
    BranchCompare(String, String),
    DeletionAdvice,
    MergeAdvice,
    RangeSummary(String, String),
    NlSearch(String),
}

impl AiState {
    pub fn new(config: &Config) -> Self {
        let available = detect_all_providers();
        let provider = config
            .ai_provider
            .as_deref()
            .and_then(AiProvider::from_str)
            .or_else(detect_provider);

        Self {
            provider,
            model: config.ai_model.clone(),
            available_providers: available,
            cache: HashMap::new(),
            loading: false,
            last_result: None,
            last_error: None,
        }
    }

    pub fn has_provider(&self) -> bool {
        self.provider.is_some()
    }

    pub fn provider_display(&self) -> String {
        match (&self.provider, &self.model) {
            (Some(p), Some(m)) => format!("{}/{}", p.name(), m),
            (Some(p), None) => p.name().to_string(),
            _ => "none".to_string(),
        }
    }

    pub fn get_cached(&self, key: &AiCacheKey) -> Option<&String> {
        self.cache.get(key)
    }

    pub fn set_cached(&mut self, key: AiCacheKey, value: String) {
        self.cache.insert(key, value);
    }

    pub fn cycle_provider(&mut self) {
        if self.available_providers.is_empty() {
            return;
        }
        let current_idx = self
            .provider
            .and_then(|p| self.available_providers.iter().position(|a| *a == p))
            .map(|i| (i + 1) % self.available_providers.len())
            .unwrap_or(0);
        self.provider = Some(self.available_providers[current_idx]);
        self.model = None;
    }
}

fn detect_all_providers() -> Vec<AiProvider> {
    let mut providers = Vec::new();
    for (provider, cmd) in [
        (AiProvider::Claude, "claude"),
        (AiProvider::Codex, "codex"),
        (AiProvider::Gemini, "gemini"),
    ] {
        if Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            providers.push(provider);
        }
    }
    providers
}

pub fn run_ai_query(provider: AiProvider, prompt: &str) -> Result<String> {
    let result = match provider {
        AiProvider::Claude => {
            let mut child = Command::new("claude")
                .arg("-p")
                .arg("--no-session-persistence")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(prompt.as_bytes())?;
            }

            let output = child.wait_with_output()?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("claude failed: {}", stderr.trim());
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        AiProvider::Codex => {
            let output = Command::new("codex")
                .arg("exec")
                .arg(prompt)
                .output()?;
            if !output.status.success() {
                anyhow::bail!("codex failed");
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        AiProvider::Gemini => {
            let mut child = Command::new("gemini")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(prompt.as_bytes())?;
            }

            let output = child.wait_with_output()?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("gemini failed: {}", stderr.trim());
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    };

    Ok(crate::ai::strip_code_blocks(&result))
}

pub fn build_prompt(action: &str, context: &str) -> String {
    match action {
        "summarize_commit" => format!(
            "Summarize this git commit in 1-2 sentences. Explain what it does and why.\n\n```diff\n{}\n```",
            context
        ),
        "explain_commit" => format!(
            "Walk through this git diff explaining each change and why it was likely made.\n\n```diff\n{}\n```",
            context
        ),
        "summarize_branch" => format!(
            "Summarize the purpose of this branch based on its commits. Be concise (2-3 sentences).\n\nBranch commits:\n{}",
            context
        ),
        "compare_branches" => format!(
            "Compare these two branches. What does each focus on? Are there potential conflicts?\n\n{}",
            context
        ),
        "deletion_advice" => format!(
            "Which of these branches are safe to delete? Consider merge status and activity.\n\n{}",
            context
        ),
        "merge_advice" => format!(
            "Which branches look ready to merge? Flag any with potential conflicts.\n\n{}",
            context
        ),
        "range_summary" => format!(
            "Summarize the changes across these commits. What was accomplished?\n\n{}",
            context
        ),
        "nl_search" => format!(
            "Given these commit messages, which ones match this query? Return matching short hashes only, one per line.\n\nQuery: {}",
            context
        ),
        _ => format!("{}\n\n{}", action, context),
    }
}
