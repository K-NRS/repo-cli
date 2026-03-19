use anyhow::Result;
use git2::Repository;

#[derive(Debug, Clone)]
pub struct BranchComparison {
    pub branch_a: String,
    pub branch_b: String,
    pub a_ahead: usize,
    pub a_behind: usize,
    pub a_unique_commits: Vec<String>,
    pub b_unique_commits: Vec<String>,
    pub diff_summary: String,
}

pub fn compare_branches(
    repo: &Repository,
    branch_a: &str,
    branch_b: &str,
) -> Result<BranchComparison> {
    let a_ref = repo.find_branch(branch_a, git2::BranchType::Local)?;
    let b_ref = repo.find_branch(branch_b, git2::BranchType::Local)?;

    let a_oid = a_ref
        .get()
        .target()
        .ok_or_else(|| anyhow::anyhow!("No target for {}", branch_a))?;
    let b_oid = b_ref
        .get()
        .target()
        .ok_or_else(|| anyhow::anyhow!("No target for {}", branch_b))?;

    let (ahead, behind) = repo.graph_ahead_behind(a_oid, b_oid)?;

    let merge_base = repo.merge_base(a_oid, b_oid)?;

    let a_commits = walk_commits(repo, a_oid, merge_base, 10)?;
    let b_commits = walk_commits(repo, b_oid, merge_base, 10)?;

    let a_tree = repo.find_commit(a_oid)?.tree()?;
    let b_tree = repo.find_commit(b_oid)?.tree()?;
    let diff = repo.diff_tree_to_tree(Some(&b_tree), Some(&a_tree), None)?;
    let stats = diff.stats()?;
    let diff_summary = format!(
        "{} files changed, {} insertions(+), {} deletions(-)",
        stats.files_changed(),
        stats.insertions(),
        stats.deletions(),
    );

    Ok(BranchComparison {
        branch_a: branch_a.to_string(),
        branch_b: branch_b.to_string(),
        a_ahead: ahead,
        a_behind: behind,
        a_unique_commits: a_commits,
        b_unique_commits: b_commits,
        diff_summary,
    })
}

fn walk_commits(
    repo: &Repository,
    from: git2::Oid,
    until: git2::Oid,
    limit: usize,
) -> Result<Vec<String>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push(from)?;
    revwalk.hide(until)?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut messages = Vec::new();
    for (count, oid_result) in revwalk.enumerate() {
        if count >= limit {
            break;
        }
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;
        let msg = commit.summary().unwrap_or("").to_string();
        messages.push(msg);
    }

    Ok(messages)
}
