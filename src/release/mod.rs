pub mod gh_cli;

use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::process::Command;

pub struct ReleaseOpts {
    pub version: Option<String>,
    pub draft: bool,
    pub dry_run: bool,
    pub notes_from_file: Option<String>,
    pub auto: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bump {
    Major,
    Minor,
    Patch,
    None,
}

/// Create a new GitHub release
pub fn create_release(opts: ReleaseOpts) -> Result<()> {
    // Resolve version
    let (version, auto_ctx) = resolve_version(&opts)?;

    let tag = if version.starts_with('v') {
        version.clone()
    } else {
        format!("v{}", version)
    };

    // Resolve notes
    let notes: Option<String> = match &opts.notes_from_file {
        Some(path) => Some(
            std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read notes file: {}", path))?,
        ),
        None => None,
    };

    // Dry run: print plan and exit
    if opts.dry_run {
        print_plan(&tag, &opts, notes.as_deref(), auto_ctx.as_ref());
        return Ok(());
    }

    println!("{}", "Checking gh CLI...".cyan());
    gh_cli::check_gh_cli()?;

    println!(
        "Creating {} release {}...",
        if opts.draft { "draft" } else { "public" },
        tag.green()
    );

    let url = gh_cli::create_release(&version, None, notes.as_deref(), opts.draft)?;

    println!("\n{} Release created!", "✓".green());
    println!("  {}", url.cyan());

    if opts.draft {
        println!(
            "\n{} This is a draft release. Publish it from GitHub when ready.",
            "→".yellow()
        );
    } else {
        println!(
            "\n{} GitHub Actions will now build and attach binaries.",
            "→".yellow()
        );
    }

    Ok(())
}

struct AutoContext {
    from_tag: String,
    bump: Bump,
    commits: Vec<String>,
}

fn resolve_version(opts: &ReleaseOpts) -> Result<(String, Option<AutoContext>)> {
    match (&opts.version, opts.auto) {
        (Some(_), true) => bail!("--auto cannot be combined with an explicit version"),
        (Some(v), false) => Ok((v.clone(), None)),
        (None, true) => {
            let (from_tag, current) = latest_tag_and_version()?;
            let commits = commits_since(&from_tag)?;
            if commits.is_empty() {
                bail!("no commits since {}", from_tag);
            }
            let bump = classify_bump(&commits);
            if bump == Bump::None {
                bail!(
                    "no release-triggering commits since {} (need feat/fix/perf/refactor or BREAKING)",
                    from_tag
                );
            }
            let next = apply_bump(&current, bump)?;
            Ok((
                next,
                Some(AutoContext { from_tag, bump, commits }),
            ))
        }
        (None, false) => bail!("version required (or pass --auto)"),
    }
}

fn latest_tag_and_version() -> Result<(String, String)> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .context("Failed to run git describe")?;

    let tag = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        "v0.0.0".to_string()
    };
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    Ok((tag, version))
}

fn commits_since(tag: &str) -> Result<Vec<String>> {
    let range = format!("{}..HEAD", tag);
    let output = Command::new("git")
        .args(["log", &range, "--pretty=format:%s"])
        .output()
        .context("Failed to run git log")?;

    if !output.status.success() {
        // Fall back to full log (fresh repo with no prior tag)
        let full = Command::new("git")
            .args(["log", "--pretty=format:%s"])
            .output()
            .context("Failed to run git log fallback")?;
        return Ok(String::from_utf8_lossy(&full.stdout)
            .lines()
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

fn classify_bump(commits: &[String]) -> Bump {
    let mut best = Bump::None;
    for msg in commits {
        if msg.starts_with("BREAKING CHANGE:") || has_type_bang(msg) {
            return Bump::Major;
        }
        if has_prefix(msg, "feat") {
            best = Bump::Minor;
        } else if best == Bump::None
            && (has_prefix(msg, "fix") || has_prefix(msg, "perf") || has_prefix(msg, "refactor"))
        {
            best = Bump::Patch;
        }
    }
    best
}

/// Matches `<prefix>:` or `<prefix>(scope):`
fn has_prefix(s: &str, prefix: &str) -> bool {
    let Some(rest) = s.strip_prefix(prefix) else {
        return false;
    };
    rest.starts_with(':') || (rest.starts_with('(') && rest.contains("):"))
}

/// Matches `<type>!:` or `<type>(scope)!:` (any lowercase type)
fn has_type_bang(s: &str) -> bool {
    let type_end = s.find(|c: char| !c.is_ascii_lowercase()).unwrap_or(0);
    if type_end == 0 {
        return false;
    }
    let rest = &s[type_end..];
    rest.starts_with("!:") || (rest.starts_with('(') && rest.contains(")!:"))
}

fn apply_bump(current: &str, bump: Bump) -> Result<String> {
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() != 3 {
        bail!("unexpected version format: {}", current);
    }
    let mut major: u64 = parts[0].parse().context("invalid major")?;
    let mut minor: u64 = parts[1].parse().context("invalid minor")?;
    let mut patch: u64 = parts[2].parse().context("invalid patch")?;

    match bump {
        Bump::Major => {
            major += 1;
            minor = 0;
            patch = 0;
        }
        Bump::Minor => {
            minor += 1;
            patch = 0;
        }
        Bump::Patch => {
            patch += 1;
        }
        Bump::None => bail!("no bump"),
    }
    Ok(format!("{}.{}.{}", major, minor, patch))
}

fn print_plan(tag: &str, opts: &ReleaseOpts, notes: Option<&str>, auto: Option<&AutoContext>) {
    println!("{}", "── dry run ──".cyan().bold());
    println!("  tag:     {}", tag.green());
    println!(
        "  type:    {}",
        if opts.draft { "draft".yellow() } else { "public".green() }
    );
    if let Some(ctx) = auto {
        println!(
            "  bump:    {} (from {})",
            format!("{:?}", ctx.bump).to_lowercase().cyan(),
            ctx.from_tag.dimmed()
        );
        println!("  commits ({}):", ctx.commits.len());
        for c in &ctx.commits {
            println!("    {} {}", "·".dimmed(), c);
        }
    }
    match notes {
        Some(body) => {
            println!("  notes:   {}", "(from file)".dimmed());
            println!("{}", "─".repeat(40).dimmed());
            for line in body.lines().take(20) {
                println!("  {}", line);
            }
            let extra = body.lines().count().saturating_sub(20);
            if extra > 0 {
                println!("  {}", format!("… +{} more line(s)", extra).dimmed());
            }
            println!("{}", "─".repeat(40).dimmed());
        }
        None => {
            println!("  notes:   {}", "gh --generate-notes".dimmed());
        }
    }
    println!("\n{} Not calling gh.", "→".yellow());
}
