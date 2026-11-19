//! Generated-text publication and read-only directory checks shared by the pipelines.

use crate::url_rewriter;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn read_public(root: &Path, path: &Path) -> Result<String, String> {
    read_public_bounded(root, path, 32 * 1024 * 1024)
}

/// Read a bounded publication input below the repository with canonical LF.
pub(crate) fn read_public_bounded(
    root: &Path,
    path: &Path,
    limit: usize,
) -> Result<String, String> {
    if path.strip_prefix(root).ok().is_none_or(|relative| {
        relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|component| component == Component::ParentDir)
    }) {
        return Err(format!(
            "public artifact is outside the repository: {}",
            path.display()
        ));
    }
    for p in path.ancestors().take_while(|p| *p != root) {
        let m =
            fs::symlink_metadata(p).map_err(|e| format!("cannot inspect {}: {e}", p.display()))?;
        if m.file_type().is_symlink() {
            return Err(format!("symlink in public input: {}", p.display()));
        }
    }
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > limit as u64 {
        return Err(format!(
            "invalid or oversized public artifact {}",
            path.display()
        ));
    }
    // Git stores public text with LF even when core.autocrlf produces a CRLF
    // worktree. Fingerprints, source byte counts and output must use that same
    // canonical text on every platform; the raw file size was bounded above.
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(text.replace("\r\n", "\n"))
}

pub(crate) fn replace_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new(""));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = parent.join(format!(
        ".tmp_{name}_{}_{}.writing",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(crate) fn verify_missing_directory(directory: &Path) -> Result<(), String> {
    // Windows may report NotFound when an ancestor is a file, not a missing directory.
    for ancestor in directory.ancestors() {
        match fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() && ancestor != directory => return Ok(()),
            Ok(_) => {
                return Err(format!(
                    "cannot verify missing directory {} at {}",
                    directory.display(),
                    ancestor.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::symlink_metadata(ancestor) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(format!("cannot inspect {}: {error}", ancestor.display()));
                    }
                    Ok(_) => {
                        return Err(format!("unresolved directory path: {}", ancestor.display()));
                    }
                }
            }
            Err(error) => return Err(format!("cannot inspect {}: {error}", ancestor.display())),
        }
    }
    Err(format!("cannot verify a parent of {}", directory.display()))
}

fn normalize_generated_text(content: &str) -> String {
    let mut lines = content.lines().map(str::trim_end).collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    }
}

pub fn safe_write_file_internal(p: &std::path::Path, content_str: &str, atomic: bool) -> bool {
    let mut normalized_content = normalize_generated_text(content_str);

    // Semantic Check
    if p.exists()
        && let Ok(old_content) = std::fs::read_to_string(p)
    {
        let old_stripped = url_rewriter::semantic_content_for_path_pub(p, &old_content);
        let new_stripped = url_rewriter::semantic_content_for_path_pub(p, &normalized_content);
        if old_stripped == new_stripped {
            let normalized_old = normalize_generated_text(&old_content);
            if normalized_old == old_content {
                println!(
                    "\x1b[0;34m[INFO]\x1b[0m Skipping write for {}: No semantic changes detected.",
                    p.file_name().unwrap_or_default().to_string_lossy()
                );
                return true;
            }
            normalized_content = normalized_old;
        }
    }

    if atomic {
        if let Err(error) = replace_file(p, normalized_content.as_bytes()) {
            eprintln!(
                "[ERROR] Failed to atomically publish {}: {error}",
                p.display()
            );
            false
        } else {
            true
        }
    } else {
        match std::fs::write(p, &normalized_content) {
            Ok(_) => true,
            Err(e) => {
                eprintln!(
                    "\x1b[0;31m[ERROR]\x1b[0m Failed to write file {:?}: {:?}",
                    p, e
                );
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_generated_text, read_public_bounded, replace_file, verify_missing_directory,
    };
    use std::fs;

    #[test]
    fn missing_directory_check_distinguishes_absence_from_blocking_file() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-directory-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let missing = root.join("missing/child");
        assert!(verify_missing_directory(&missing).is_ok());
        fs::write(root.join("missing"), "not a directory").unwrap();
        assert!(verify_missing_directory(&missing).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn public_reader_keeps_limit_and_rejects_parent_traversal() {
        let base = std::env::temp_dir().join(format!(
            "script-hub-public-reader-{}-{}",
            std::process::id(),
            super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let root = base.join("public");
        fs::create_dir_all(&root).unwrap();
        let published = root.join("rules.list");
        fs::write(&published, "value\r\n").unwrap();
        let outside = base.join("outside.list");
        fs::write(&outside, "private").unwrap();
        assert_eq!(
            read_public_bounded(&root, &published, 7).unwrap(),
            "value\n"
        );
        assert!(read_public_bounded(&root, &published, 6).is_err());
        assert!(read_public_bounded(&root, &outside, 32).is_err());
        assert!(read_public_bounded(&root, &root.join("../outside.list"), 32).is_err());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn generated_text_has_clean_line_endings_and_one_final_newline() {
        assert_eq!(
            normalize_generated_text("# comment \r\nvalue\t\r\n\r\n"),
            "# comment\nvalue\n"
        );
        assert_eq!(normalize_generated_text(""), "");
    }

    #[test]
    fn failed_atomic_replace_removes_only_its_staging_file() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-atomic-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let destination = root.join("directory");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("keep"), "keep").unwrap();
        assert!(replace_file(&destination, b"new data").is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(destination.join("keep")).unwrap(),
            "keep"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
