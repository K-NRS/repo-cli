use colored::Colorize;
use std::path::Path;
use std::process::Command;

/// Fetch from all remotes using git CLI, returning any errors as warnings
pub fn fetch_all_remotes(repo_path: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["-C", &repo_path.display().to_string(), "remote"])
        .output();

    let remotes = match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(String::from)
                .collect::<Vec<_>>()
        }
        _ => return vec!["failed to list remotes".to_string()],
    };

    let mut warnings = Vec::new();

    for remote in remotes {
        let fetch = Command::new("git")
            .args(["-C", &repo_path.display().to_string(), "fetch", &remote, "--quiet"])
            .output();

        match fetch {
            Ok(o) if !o.status.success() => {
                let err = String::from_utf8_lossy(&o.stderr);
                let msg = err.lines().next().unwrap_or("fetch failed");
                warnings.push(format!("{}: {}", remote, msg.trim()));
            }
            Err(e) => warnings.push(format!("{}: {}", remote, e)),
            _ => {}
        }
    }

    warnings
}

/// Print fetch warnings to stderr
pub fn print_fetch_warnings(warnings: &[String]) {
    for warning in warnings {
        eprintln!("{} fetch: {}", "⚠".yellow(), warning);
    }
}
