mod provider;
mod claude;
mod codex;
mod gemini;

pub use provider::{AiProvider, detect_provider, generate_commit_message};
