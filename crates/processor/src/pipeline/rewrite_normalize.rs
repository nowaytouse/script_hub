//! Offline migration of native module outputs through the shared serializer.
use crate::promax::artifact::{ArtifactKind, GeneratedArtifact, publish_generated_artifacts};
use std::path::Path;

pub fn run(root: &Path) -> Result<usize, String> {
    let mut artifacts = Vec::new();
    for (relative, extension, kind) in [
        ("modules/surge", "sgmodule", ArtifactKind::Surge),
        ("modules/shadowrocket", "module", ArtifactKind::Shadowrocket),
        (
            "modules/source/build/promax_splits",
            "sgmodule",
            ArtifactKind::Surge,
        ),
    ] {
        let directory = root.join(relative);
        if !directory.exists() {
            crate::verify_missing_directory(&directory)?;
            continue;
        }
        for entry in walkdir::WalkDir::new(&directory) {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().is_symlink() {
                return Err("symlink in module normalization inputs".into());
            }
            if !entry.file_type().is_file()
                || entry.path().extension().is_none_or(|ext| ext != extension)
            {
                continue;
            }
            let content = std::fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
            let normalized = crate::rewrite_syntax::normalize_module(&content).map_err(|e| {
                format!(
                    "{}: {e}",
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap_or(entry.path())
                        .display()
                )
            })?;
            if content != normalized {
                artifacts.push(GeneratedArtifact {
                    target: entry.path().to_owned(),
                    content: normalized,
                    kind,
                });
            }
        }
    }
    let count = artifacts.len();
    if !artifacts.is_empty() {
        publish_generated_artifacts(root, &artifacts)?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_migration_is_idempotent_and_invalid_input_leaves_outputs_intact() {
        let root = std::env::temp_dir().join(format!("rewrite-normalize-{}", std::process::id()));
        let folder = root.join("modules/surge");
        std::fs::create_dir_all(&folder).unwrap();
        let valid = folder.join("valid.sgmodule");
        let invalid = folder.join("invalid.sgmodule");
        let source = "#!name=Synthetic\n[Body Rewrite]\nhttp-response ^https://example.test offset_ms\":\\d+ offset_ms\":99\n";
        std::fs::write(&valid, source).unwrap();
        std::fs::write(
            &invalid,
            "#!name=Synthetic\n[Body Rewrite]\nhttp-response ^https://example.test search\n",
        )
        .unwrap();
        assert!(run(&root).is_err());
        assert_eq!(std::fs::read_to_string(&valid).unwrap(), source);
        std::fs::remove_file(&invalid).unwrap();
        assert_eq!(run(&root).unwrap(), 1);
        assert_eq!(run(&root).unwrap(), 0);
        let output = std::fs::read_to_string(&valid).unwrap();
        assert!(crate::promax::validation::validate_surge_module(&output).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
