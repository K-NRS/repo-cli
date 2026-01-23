use anyhow::{Context, Result};
use colored::Colorize;
use git2::{FetchOptions, RemoteCallbacks, Repository};

/// Fetch from all remotes, returning any errors as warnings
pub fn fetch_all_remotes(repo: &Repository) -> Vec<String> {
    let remotes = match repo.remotes() {
        Ok(r) => r,
        Err(e) => return vec![format!("Failed to list remotes: {}", e)],
    };

    let mut warnings = Vec::new();

    for remote_name in remotes.iter().flatten() {
        if let Err(e) = fetch_remote(repo, remote_name) {
            warnings.push(format!("{}: {}", remote_name, e));
        }
    }

    warnings
}

fn fetch_remote(repo: &Repository, remote_name: &str) -> Result<()> {
    let mut remote = repo
        .find_remote(remote_name)
        .context("remote not found")?;

    let mut callbacks = RemoteCallbacks::new();

    // Use credential helper for SSH/HTTPS auth
    callbacks.credentials(|_url, username_from_url, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            let username = username_from_url.unwrap_or("git");
            git2::Cred::ssh_key_from_agent(username)
        } else if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            git2::Cred::credential_helper(
                &repo.config()?,
                _url,
                username_from_url,
            )
        } else if allowed_types.contains(git2::CredentialType::DEFAULT) {
            git2::Cred::default()
        } else {
            Err(git2::Error::from_str("no credentials available"))
        }
    });

    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);
    fetch_opts.download_tags(git2::AutotagOption::Auto);

    // Fetch all branches
    let refspecs: Vec<String> = remote
        .fetch_refspecs()?
        .iter()
        .flatten()
        .map(String::from)
        .collect();

    let refspec_refs: Vec<&str> = refspecs.iter().map(|s| s.as_str()).collect();

    remote
        .fetch(&refspec_refs, Some(&mut fetch_opts), None)
        .context("network or auth error")?;

    Ok(())
}

/// Print fetch warnings to stderr
pub fn print_fetch_warnings(warnings: &[String]) {
    for warning in warnings {
        eprintln!("{} fetch: {}", "⚠".yellow(), warning);
    }
}
