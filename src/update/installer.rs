use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Extract archive and return path to the binary inside
pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(dest_dir)?;

    let extension = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if archive_path.to_string_lossy().ends_with(".tar.gz") {
        extract_tar_gz(archive_path, dest_dir)
    } else if extension == "zip" {
        extract_zip(archive_path, dest_dir)
    } else {
        Err(anyhow!("Unknown archive format: {:?}", archive_path))
    }
}

/// Extract .tar.gz archive
fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<PathBuf> {
    use flate2::read::GzDecoder;
    use std::io::BufReader;

    let file = File::open(archive_path).context("Failed to open archive")?;
    let reader = BufReader::new(file);
    let decoder = GzDecoder::new(reader);
    let mut archive = tar::Archive::new(decoder);

    archive.unpack(dest_dir).context("Failed to extract tar.gz")?;

    find_binary_in_dir(dest_dir)
}

/// Extract .zip archive
fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<PathBuf> {
    let file = File::open(archive_path).context("Failed to open archive")?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to read zip archive")?;

    archive.extract(dest_dir).context("Failed to extract zip")?;

    find_binary_in_dir(dest_dir)
}

/// Find the repo binary in extracted directory
fn find_binary_in_dir(dir: &Path) -> Result<PathBuf> {
    let binary_name = if cfg!(windows) { "repo.exe" } else { "repo" };

    // Check directly in dir
    let direct = dir.join(binary_name);
    if direct.exists() {
        return Ok(direct);
    }

    // Check one level deep (common for archives)
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let nested = path.join(binary_name);
            if nested.exists() {
                return Ok(nested);
            }
        }
    }

    Err(anyhow!("Binary '{}' not found in archive", binary_name))
}

/// Replace current binary with new one
pub fn replace_binary(new_binary: &Path) -> Result<()> {
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;

    // On Unix, we can replace the running binary by moving
    // On Windows, we need to rename the old binary first
    #[cfg(windows)]
    {
        let backup = current_exe.with_extension("exe.bak");
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(&current_exe, &backup).context("Failed to backup current binary")?;
    }

    // Copy new binary to current location
    fs::copy(new_binary, &current_exe).context("Failed to install new binary")?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&current_exe)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&current_exe, perms)?;
    }

    Ok(())
}

/// Get a temporary directory for update operations
pub fn get_temp_dir() -> Result<PathBuf> {
    let temp = std::env::temp_dir().join("repo-cli-update");
    fs::create_dir_all(&temp)?;
    Ok(temp)
}

/// Clean up temporary update files
pub fn cleanup_temp_dir() -> Result<()> {
    let temp = std::env::temp_dir().join("repo-cli-update");
    if temp.exists() {
        fs::remove_dir_all(&temp)?;
    }
    Ok(())
}
