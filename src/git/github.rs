use anyhow::{Context, Result};
use git2::Repository;
use serde::Deserialize;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Deserialize)]
struct GithubRepo {
    stargazers_count: u32,
    forks_count: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Stargazer {
    pub login: String,
    pub html_url: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ForkOwner {
    login: String,
    html_url: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ForkResponse {
    full_name: String,
    html_url: String,
    owner: ForkOwner,
    stargazers_count: u32,
}

#[derive(Debug, Clone)]
pub struct Fork {
    pub repo_name: String,
    pub repo_url: String,
    pub owner: String,
    pub owner_url: String,
    pub stars: u32,
}

#[derive(Debug, Clone)]
pub struct GithubStats {
    pub stars: u32,
    pub forks: u32,
}

pub fn get_github_stats(repo: &Repository) -> Option<GithubStats> {
    let (owner, name) = parse_github_remote(repo)?;
    fetch_repo_stats(&owner, &name).ok()
}

pub fn get_stargazers(repo: &Repository) -> Result<Vec<Stargazer>> {
    let (owner, name) = parse_github_remote(repo).context("Not a GitHub repository")?;
    fetch_stargazers(&owner, &name)
}

pub fn get_forks(repo: &Repository) -> Result<Vec<Fork>> {
    let (owner, name) = parse_github_remote(repo).context("Not a GitHub repository")?;
    fetch_forks(&owner, &name)
}

pub fn parse_github_remote(repo: &Repository) -> Option<(String, String)> {
    let remote = repo.find_remote("origin").ok()?;
    let url = remote.url()?;
    parse_github_url(url)
}

fn parse_github_url(url: &str) -> Option<(String, String)> {
    // Handle SSH: git@github.com:owner/repo.git
    if url.starts_with("git@github.com:") {
        let path = url.strip_prefix("git@github.com:")?;
        let path = path.strip_suffix(".git").unwrap_or(path);
        let mut parts = path.splitn(2, '/');
        let owner = parts.next()?;
        let name = parts.next()?;
        return Some((owner.to_string(), name.to_string()));
    }

    // Handle HTTPS: https://github.com/owner/repo.git
    if url.contains("github.com") {
        let url = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
        let url = url.strip_prefix("github.com/")?;
        let url = url.strip_suffix(".git").unwrap_or(url);
        let mut parts = url.splitn(2, '/');
        let owner = parts.next()?;
        let name = parts.next()?;
        return Some((owner.to_string(), name.to_string()));
    }

    None
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::new()
}

fn fetch_repo_stats(owner: &str, name: &str) -> Result<GithubStats> {
    let url = format!("https://api.github.com/repos/{}/{}", owner, name);
    let resp: GithubRepo = client()
        .get(&url)
        .header("User-Agent", "repo-cli")
        .timeout(TIMEOUT)
        .send()?
        .json()?;
    Ok(GithubStats {
        stars: resp.stargazers_count,
        forks: resp.forks_count,
    })
}

fn fetch_stargazers(owner: &str, name: &str) -> Result<Vec<Stargazer>> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/stargazers?per_page=100",
        owner, name
    );
    let resp: Vec<Stargazer> = client()
        .get(&url)
        .header("User-Agent", "repo-cli")
        .timeout(TIMEOUT)
        .send()?
        .json()?;
    Ok(resp)
}

fn fetch_forks(owner: &str, name: &str) -> Result<Vec<Fork>> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/forks?sort=stargazers&per_page=100",
        owner, name
    );
    let resp: Vec<ForkResponse> = client()
        .get(&url)
        .header("User-Agent", "repo-cli")
        .timeout(TIMEOUT)
        .send()?
        .json()?;
    Ok(resp
        .into_iter()
        .map(|f| Fork {
            repo_name: f.full_name,
            repo_url: f.html_url,
            owner: f.owner.login,
            owner_url: f.owner.html_url,
            stars: f.stargazers_count,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_url() {
        let result = parse_github_url("git@github.com:K-NRS/repo-cli.git");
        assert_eq!(result, Some(("K-NRS".to_string(), "repo-cli".to_string())));
    }

    #[test]
    fn test_parse_https_url() {
        let result = parse_github_url("https://github.com/K-NRS/repo-cli.git");
        assert_eq!(result, Some(("K-NRS".to_string(), "repo-cli".to_string())));
    }

    #[test]
    fn test_parse_https_no_git_suffix() {
        let result = parse_github_url("https://github.com/K-NRS/repo-cli");
        assert_eq!(result, Some(("K-NRS".to_string(), "repo-cli".to_string())));
    }
}
