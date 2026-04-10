use chrono::{DateTime, Local, NaiveDate, TimeZone};

use crate::models::CommitInfo;
use crate::workspace::RepoSnapshot;

#[derive(Debug, Clone, PartialEq)]
pub enum RepoStatusKind {
    Dirty,
    Clean,
    Ahead,
    Behind,
    Stale,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceFilter {
    Message(String),
    Author(String),
    DateRange {
        from: Option<DateTime<Local>>,
        to: Option<DateTime<Local>>,
    },
    RepoName(String),
    RepoStatus(RepoStatusKind),
    /// Case-insensitive substring match across repo name + commit message + author
    FullText(String),
}

const PREFIXES: &[&str] = &[
    "msg:", "author:", "date:", "repo:", "status:", "text:",
];

pub fn parse_filters(input: &str) -> Vec<WorkspaceFilter> {
    let mut filters = Vec::new();
    let mut remaining = input.trim();

    while !remaining.is_empty() {
        if let Some(rest) = remaining.strip_prefix("msg:") {
            let (value, next) = extract_value(rest);
            if !value.is_empty() {
                filters.push(WorkspaceFilter::Message(value));
            }
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("author:") {
            let (value, next) = extract_value(rest);
            if !value.is_empty() {
                filters.push(WorkspaceFilter::Author(value));
            }
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("date:") {
            let (value, next) = extract_value(rest);
            let (from, to) = parse_date_range(&value);
            if from.is_some() || to.is_some() {
                filters.push(WorkspaceFilter::DateRange { from, to });
            }
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("repo:") {
            let (value, next) = extract_value(rest);
            if !value.is_empty() {
                filters.push(WorkspaceFilter::RepoName(value));
            }
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("status:") {
            let (value, next) = extract_value(rest);
            if let Some(kind) = parse_status(&value) {
                filters.push(WorkspaceFilter::RepoStatus(kind));
            }
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("text:") {
            let (value, next) = extract_value(rest);
            if !value.is_empty() {
                filters.push(WorkspaceFilter::FullText(value));
            }
            remaining = next;
        } else {
            // Bare terms become FullText — searches across everything
            let (value, next) = extract_value(remaining);
            if !value.is_empty() {
                filters.push(WorkspaceFilter::FullText(value));
            }
            remaining = next;
        }
    }

    filters
}

fn extract_value(s: &str) -> (String, &str) {
    let end = PREFIXES
        .iter()
        .filter_map(|p| s.find(p))
        .filter(|&pos| pos > 0)
        .min()
        .unwrap_or(s.len());
    (s[..end].trim().to_string(), &s[end..])
}

fn parse_status(s: &str) -> Option<RepoStatusKind> {
    match s.trim().to_lowercase().as_str() {
        "dirty" | "modified" => Some(RepoStatusKind::Dirty),
        "clean" => Some(RepoStatusKind::Clean),
        "ahead" => Some(RepoStatusKind::Ahead),
        "behind" => Some(RepoStatusKind::Behind),
        "stale" => Some(RepoStatusKind::Stale),
        _ => None,
    }
}

fn parse_date_range(s: &str) -> (Option<DateTime<Local>>, Option<DateTime<Local>>) {
    let parts: Vec<&str> = s.split("..").collect();
    let from = parts.first().and_then(|s| parse_date(s));
    let to = parts.get(1).and_then(|s| parse_date(s));
    (from, to)
}

fn parse_date(s: &str) -> Option<DateTime<Local>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single();
    }
    if let Ok(d) = NaiveDate::parse_from_str(&format!("{}-01", s), "%Y-%m-%d") {
        return Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single();
    }
    None
}

/// Filter repos by repo-level predicates (name, status)
pub fn filter_repos<'a>(
    repos: &'a [RepoSnapshot],
    filters: &[WorkspaceFilter],
) -> Vec<&'a RepoSnapshot> {
    repos
        .iter()
        .filter(|r| filters.iter().all(|f| repo_matches(r, f)))
        .collect()
}

fn repo_matches(repo: &RepoSnapshot, filter: &WorkspaceFilter) -> bool {
    match filter {
        WorkspaceFilter::RepoName(name) => repo
            .name
            .to_lowercase()
            .contains(&name.to_lowercase()),
        WorkspaceFilter::RepoStatus(kind) => match kind {
            RepoStatusKind::Dirty => repo.is_dirty(),
            RepoStatusKind::Clean => !repo.is_dirty(),
            RepoStatusKind::Ahead => repo.ahead() > 0,
            RepoStatusKind::Behind => repo.behind() > 0,
            RepoStatusKind::Stale => repo.stale,
        },
        WorkspaceFilter::FullText(term) => {
            let t = term.to_lowercase();
            repo.name.to_lowercase().contains(&t)
                || repo.branch.to_lowercase().contains(&t)
                || repo
                    .recent_commits
                    .iter()
                    .any(|c| commit_matches_text(c, &t))
        }
        // Commit-level filters don't exclude repos by themselves
        WorkspaceFilter::Message(_)
        | WorkspaceFilter::Author(_)
        | WorkspaceFilter::DateRange { .. } => true,
    }
}

fn commit_matches_text(commit: &CommitInfo, term_lower: &str) -> bool {
    commit.message.to_lowercase().contains(term_lower)
        || commit.author.to_lowercase().contains(term_lower)
}

/// Filter a commit (given its originating repo name) by commit-level predicates
pub fn commit_matches(
    commit: &CommitInfo,
    repo_name: &str,
    filters: &[WorkspaceFilter],
) -> bool {
    filters.iter().all(|f| match f {
        WorkspaceFilter::Message(term) => {
            commit.message.to_lowercase().contains(&term.to_lowercase())
        }
        WorkspaceFilter::Author(name) => {
            commit.author.to_lowercase().contains(&name.to_lowercase())
        }
        WorkspaceFilter::DateRange { from, to } => {
            from.map_or(true, |f| commit.time >= f)
                && to.map_or(true, |t| commit.time <= t)
        }
        WorkspaceFilter::RepoName(name) => {
            repo_name.to_lowercase().contains(&name.to_lowercase())
        }
        WorkspaceFilter::FullText(term) => {
            let t = term.to_lowercase();
            repo_name.to_lowercase().contains(&t) || commit_matches_text(commit, &t)
        }
        // Repo-status filters apply at repo level, not per-commit
        WorkspaceFilter::RepoStatus(_) => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_text_as_fulltext() {
        let f = parse_filters("auth refactor");
        assert_eq!(f, vec![WorkspaceFilter::FullText("auth refactor".into())]);
    }

    #[test]
    fn parses_status_dirty() {
        let f = parse_filters("status:dirty");
        assert_eq!(f, vec![WorkspaceFilter::RepoStatus(RepoStatusKind::Dirty)]);
    }

    #[test]
    fn parses_combined() {
        let f = parse_filters("repo:cli status:behind author:keren");
        assert_eq!(f.len(), 3);
    }

    #[test]
    fn ignores_unknown_status() {
        let f = parse_filters("status:weird");
        assert!(f.is_empty());
    }
}
