use std::io::{self, Write};

/// Set the terminal window/tab title via OSC escape sequence.
pub fn set_title(title: &str) {
    let _ = write!(io::stdout(), "\x1b]0;{}\x07", title);
    let _ = io::stdout().flush();
}

/// Restore the terminal title to the default (empty resets to shell default).
pub fn restore_title() {
    // Setting empty title lets the terminal/shell reclaim the title
    let _ = write!(io::stdout(), "\x1b]0;\x07");
    let _ = io::stdout().flush();
}

/// Derive a short repo name from a git2::Repository (uses workdir basename).
pub fn repo_display_name(repo: &git2::Repository) -> String {
    repo.workdir()
        .or_else(|| Some(repo.path()))
        .and_then(|p| p.file_name().or_else(|| p.parent().and_then(|pp| pp.file_name())))
        .and_then(|n| n.to_str())
        .unwrap_or("repo")
        .to_string()
}
