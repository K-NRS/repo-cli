use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::models::CommitInfo;
use super::actions::{RebaseAction, TodoEntry};
use super::split::generate_patch_for_hunks;

pub fn execute_craft_plan(
    repo_path: &Path,
    commits: &[CommitInfo],
    entries: &[TodoEntry],
    hunks_cache: &std::collections::HashMap<usize, Vec<super::split::Hunk>>,
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let tmp_dir = std::env::temp_dir().join(format!("repo-craft-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir)?;

    // Determine base: parent of oldest commit in the plan
    let oldest_idx = entries.iter().map(|e| e.original_idx).max().unwrap();
    let has_root = commits[oldest_idx].parents.is_empty();
    let base_sha = if has_root {
        None
    } else {
        Some(commits[oldest_idx].parents[0].to_string())
    };

    // Build ordered entries (oldest first = highest idx first)
    let mut ordered: Vec<&TodoEntry> = entries.iter().collect();
    ordered.sort_by(|a, b| b.original_idx.cmp(&a.original_idx));

    // Collect reword messages (counter-based serving)
    let mut editor_messages: Vec<String> = Vec::new();
    // Track which commits need split automation
    let mut split_entries: Vec<(String, Vec<(Vec<usize>, String)>)> = Vec::new();

    let seq_script = write_sequence_editor(&tmp_dir, commits, &ordered)?;

    // Prepare messages for reword/squash actions
    for entry in &ordered {
        match &entry.action {
            RebaseAction::Reword(msg) => {
                editor_messages.push(msg.clone());
            }
            RebaseAction::Squash { message: Some(msg), .. } => {
                editor_messages.push(msg.clone());
            }
            RebaseAction::Split { groups } => {
                let sha = commits[entry.original_idx].id.to_string();
                let hunk_groups: Vec<(Vec<usize>, String)> = groups
                    .iter()
                    .map(|g| (g.hunk_indices.clone(), g.message.clone()))
                    .collect();
                split_entries.push((sha, hunk_groups));
            }
            _ => {}
        }
    }

    let msg_script = write_commit_editor(&tmp_dir, &editor_messages)?;

    // Write split automation scripts if needed
    let split_script = if !split_entries.is_empty() {
        Some(write_split_automation(&tmp_dir, hunks_cache, commits, &split_entries)?)
    } else {
        None
    };

    // Run rebase
    let mut args = vec![
        "-C".to_string(),
        repo_path.to_string_lossy().to_string(),
        "rebase".to_string(),
        "-i".to_string(),
    ];

    match &base_sha {
        Some(sha) => args.push(sha.clone()),
        None => args.push("--root".to_string()),
    }

    let output = Command::new("git")
        .args(&args)
        .env("GIT_SEQUENCE_EDITOR", &seq_script)
        .env("GIT_EDITOR", &msg_script)
        .output()?;

    // If rebase stopped for edit (split), run the auto-split script
    if let Some(ref script) = split_script {
        let status_output = Command::new("git")
            .args(["-C", &repo_path.to_string_lossy(), "status"])
            .output()?;
        let status_str = String::from_utf8_lossy(&status_output.stdout);

        if status_str.contains("interactive rebase") || status_str.contains("edit") {
            run_split_automation(repo_path, script)?;
        }
    }

    // cleanup
    fs::remove_dir_all(&tmp_dir).ok();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stderr.contains("CONFLICT") || stderr.contains("could not apply") {
            bail!(
                "rebase conflict:\n{}\n{}\nresolve manually:\n  git rebase --continue\n  git rebase --abort",
                stdout, stderr
            );
        }

        bail!("rebase failed:\n{}\n{}", stdout, stderr);
    }

    Ok(())
}

fn write_sequence_editor(
    tmp_dir: &Path,
    commits: &[CommitInfo],
    ordered: &[&TodoEntry],
) -> Result<PathBuf> {
    let script_path = tmp_dir.join("seq-editor.sh");

    // Build sed commands for each entry
    let mut sed_cmds: Vec<String> = Vec::new();

    for entry in ordered {
        let sha = &commits[entry.original_idx].short_id;
        match &entry.action {
            RebaseAction::Pick => {} // default, no change needed
            RebaseAction::Reword(_) => {
                sed_cmds.push(format!("s/^pick {sha}/reword {sha}/"));
            }
            RebaseAction::Squash { .. } => {
                sed_cmds.push(format!("s/^pick {sha}/squash {sha}/"));
            }
            RebaseAction::Fixup { .. } => {
                sed_cmds.push(format!("s/^pick {sha}/fixup {sha}/"));
            }
            RebaseAction::Drop => {
                sed_cmds.push(format!("s/^pick {sha}/drop {sha}/"));
            }
            RebaseAction::Split { .. } => {
                sed_cmds.push(format!("s/^pick {sha}/edit {sha}/"));
            }
            RebaseAction::Edit => {
                sed_cmds.push(format!("s/^pick {sha}/edit {sha}/"));
            }
        }
    }

    // Handle reordering: we also need to reorder the lines
    // Build the desired order of short SHAs
    let desired_order: Vec<String> = ordered
        .iter()
        .map(|e| commits[e.original_idx].short_id.clone())
        .collect();

    let sed_expr = if sed_cmds.is_empty() {
        String::new()
    } else {
        sed_cmds.join("; ")
    };

    // Write a script that first applies action changes, then reorders
    let reorder_awk = build_reorder_script(&desired_order);

    let script = if sed_expr.is_empty() {
        format!("#!/bin/sh\n{}\n", reorder_awk)
    } else {
        format!(
            "#!/bin/sh\nsed -i.bak '{sed_expr}' \"$1\"\n{}\n",
            reorder_awk
        )
    };

    fs::write(&script_path, &script)?;
    make_executable(&script_path)?;

    Ok(script_path)
}

fn build_reorder_script(desired_order: &[String]) -> String {
    // Build an awk script that reorders the todo lines
    // We write the desired order to a temp file and use it to sort
    let mut script = String::new();

    // Create an awk script that assigns order based on SHA position
    script.push_str("awk '{\n");
    for (i, sha) in desired_order.iter().enumerate() {
        script.push_str(&format!("  if ($0 ~ /{}/) {{ order[NR] = {}; lines[NR] = $0 }}\n", sha, i));
    }
    script.push_str("  if (!(NR in order)) { order[NR] = NR + 1000; lines[NR] = $0 }\n");
    script.push_str("}\nEND {\n");
    script.push_str("  n = asorti(order, sorted, \"@val_num_asc\")\n");
    script.push_str("  for (i = 1; i <= n; i++) print lines[sorted[i]]\n");
    script.push_str("}' \"$1\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"\n");

    script
}

fn write_commit_editor(tmp_dir: &Path, messages: &[String]) -> Result<PathBuf> {
    let script_path = tmp_dir.join("msg-editor.sh");
    let counter_path = tmp_dir.join("counter");

    fs::write(&counter_path, "0")?;

    for (i, msg) in messages.iter().enumerate() {
        let msg_path = tmp_dir.join(format!("msg_{}", i));
        fs::write(&msg_path, msg)?;
    }

    let counter_str = counter_path.to_string_lossy();
    let tmp_str = tmp_dir.to_string_lossy();

    let script = format!(
        r#"#!/bin/sh
COUNTER_FILE="{counter_str}"
IDX=$(cat "$COUNTER_FILE")
MSG_FILE="{tmp_str}/msg_$IDX"
if [ -f "$MSG_FILE" ]; then
    cp "$MSG_FILE" "$1"
fi
echo $((IDX + 1)) > "$COUNTER_FILE"
"#
    );

    fs::write(&script_path, &script)?;
    make_executable(&script_path)?;

    Ok(script_path)
}

fn write_split_automation(
    tmp_dir: &Path,
    hunks_cache: &std::collections::HashMap<usize, Vec<super::split::Hunk>>,
    commits: &[CommitInfo],
    split_entries: &[(String, Vec<(Vec<usize>, String)>)],
) -> Result<PathBuf> {
    let script_path = tmp_dir.join("auto-split.sh");
    let patches_dir = tmp_dir.join("patches");
    fs::create_dir_all(&patches_dir)?;

    let mut script = String::from("#!/bin/sh\nset -e\n");
    script.push_str("# Auto-split script generated by repo craft\n\n");

    for (sha, groups) in split_entries {
        // Find the commit index
        let commit_idx = commits
            .iter()
            .position(|c| c.id.to_string() == *sha)
            .unwrap_or(0);

        if let Some(hunks) = hunks_cache.get(&commit_idx) {
            // Reset the current commit
            script.push_str("git reset HEAD^\n\n");

            for (group_idx, (hunk_indices, message)) in groups.iter().enumerate() {
                let patch_file = patches_dir.join(format!("patch_{}_{}.patch", commit_idx, group_idx));
                let patch_content = generate_patch_for_hunks(hunks, hunk_indices);
                fs::write(&patch_file, &patch_content)?;

                let patch_path = patch_file.to_string_lossy();
                let escaped_msg = message.replace('\'', "'\\''");
                script.push_str(&format!(
                    "git apply --cached '{}'\ngit commit -m '{}'\n\n",
                    patch_path, escaped_msg
                ));
            }

            script.push_str("git rebase --continue\n");
        }
    }

    fs::write(&script_path, &script)?;
    make_executable(&script_path)?;

    Ok(script_path)
}

fn run_split_automation(repo_path: &Path, script: &Path) -> Result<()> {
    let output = Command::new("sh")
        .arg(script)
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("split automation failed:\n{}", stderr);
    }

    Ok(())
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}
