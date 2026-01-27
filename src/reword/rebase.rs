use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Run `git rebase -i` with custom sequence and commit editors.
///
/// `base_sha`: parent of oldest commit to reword, or None for `--root`
/// `selected_shas`: short SHAs to mark as `reword`
/// `messages`: vec of (full_sha, new_message) pairs — oldest-first order
pub fn run_interactive_rebase(
    repo_path: &Path,
    base_sha: Option<&str>,
    selected_shas: &[String],
    messages: &[(String, String)],
) -> Result<()> {
    let tmp_dir = std::env::temp_dir().join(format!("repo-reword-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir)?;

    let seq_script = write_sequence_editor(&tmp_dir, selected_shas)?;
    let msg_script = write_commit_editor(&tmp_dir, messages)?;

    let mut args = vec![
        "-C".to_string(),
        repo_path.to_string_lossy().to_string(),
        "rebase".to_string(),
        "-i".to_string(),
    ];

    match base_sha {
        Some(sha) => args.push(sha.to_string()),
        None => args.push("--root".to_string()),
    }

    let output = Command::new("git")
        .args(&args)
        .env("GIT_SEQUENCE_EDITOR", &seq_script)
        .env("GIT_EDITOR", &msg_script)
        .output()?;

    // cleanup
    fs::remove_dir_all(&tmp_dir).ok();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stderr.contains("CONFLICT") || stderr.contains("could not apply") {
            bail!(
                "rebase failed with conflict:\n{}\n{}\nresolve manually, then:\n  git rebase --continue\nor:\n  git rebase --abort",
                stdout,
                stderr
            );
        }

        bail!("rebase failed:\n{}\n{}", stdout, stderr);
    }

    Ok(())
}

/// Write a shell script that replaces `pick` with `reword` for selected SHAs.
fn write_sequence_editor(tmp_dir: &Path, selected_shas: &[String]) -> Result<PathBuf> {
    let script_path = tmp_dir.join("seq-editor.sh");

    let sed_cmds: Vec<String> = selected_shas
        .iter()
        .map(|sha| format!("s/^pick {sha}/reword {sha}/"))
        .collect();

    let sed_expr = sed_cmds.join("; ");
    let script = format!("#!/bin/sh\nsed -i.bak '{sed_expr}' \"$1\"\n");

    fs::write(&script_path, &script)?;
    make_executable(&script_path)?;

    Ok(script_path)
}

/// Write numbered message files and a script that serves them sequentially.
///
/// During `git rebase -i`, each `reword` commit invokes GIT_EDITOR once.
/// We write each new message to `msg_0`, `msg_1`, ... and use a counter file
/// to track which message to serve next.
fn write_commit_editor(tmp_dir: &Path, messages: &[(String, String)]) -> Result<PathBuf> {
    let script_path = tmp_dir.join("msg-editor.sh");
    let counter_path = tmp_dir.join("counter");

    // Initialize counter to 0
    fs::write(&counter_path, "0")?;

    // Write each message as a separate file: msg_0, msg_1, ...
    for (i, (_sha, msg)) in messages.iter().enumerate() {
        let msg_path = tmp_dir.join(format!("msg_{}", i));
        fs::write(&msg_path, msg)?;
    }

    let counter_str = counter_path.to_string_lossy();
    let tmp_str = tmp_dir.to_string_lossy();

    // Script: read counter, copy corresponding message file to $1, increment counter
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
