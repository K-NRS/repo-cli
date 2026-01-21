use anyhow::{Context, Result};
use git2::Repository;
use std::path::Path;

pub fn open_repo(path: Option<&Path>) -> Result<Repository> {
    match path {
        Some(p) => Repository::open(p).context("Failed to open repository"),
        None => Repository::open_from_env().or_else(|_| {
            Repository::discover(".").context("Not a git repository (or any parent)")
        }),
    }
}
