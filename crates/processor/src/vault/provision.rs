//! First-time encrypted publication; plaintext source files are never removed here.

use super::{
    Client, Library, MAX_TEMPLATES, VaultError, create, read_bounded, template, unlock,
    write_private,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use zeroize::Zeroizing;

fn absolute(path: &Path) -> Result<PathBuf, VaultError> {
    if path.file_name().is_none() || path.components().any(|part| part == Component::ParentDir) {
        return Err(VaultError::Format);
    }
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()
            .map_err(|_| VaultError::Io)?
            .join(path))
    }
}

fn unused(path: &Path) -> Result<(), VaultError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(VaultError::AlreadyExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(VaultError::Io),
    }
}

fn local_backup_only(path: &Path) -> Result<(), VaultError> {
    for ancestor in path.parent().ok_or(VaultError::Format)?.ancestors() {
        match fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => return Err(VaultError::Format),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(VaultError::Io),
        }
    }
    Ok(())
}

fn verify(path: &Path, expected: &Library, password: &str) -> Result<(), VaultError> {
    let ciphertext = read_bounded(path)?;
    let recovered = unlock(&ciphertext, password)?;
    let actual = recovered.library();
    if actual.templates.len() != expected.templates.len()
        || !actual
            .templates
            .iter()
            .zip(&expected.templates)
            .all(|(actual, expected)| {
                actual.client == expected.client
                    && actual.name == expected.name
                    && actual.content.as_bytes() == expected.content.as_bytes()
            })
    {
        return Err(VaultError::Io);
    }
    Ok(())
}

/// Creates separate encrypted original and sanitized libraries, then verifies both from disk.
pub fn provision(
    paths: &[(PathBuf, Client)],
    public_path: &Path,
    backup_path: &Path,
    password: &str,
) -> Result<(), VaultError> {
    local_backup_only(&absolute(backup_path)?)?;
    provision_to(paths, public_path, backup_path, password)
}

pub(crate) fn provision_managed(
    paths: &[(PathBuf, Client)],
    project: &Path,
    password: &str,
) -> Result<(), VaultError> {
    let store = super::managed::ManagedStore::new(project)?;
    store.restore()?;
    provision_to(paths, &store.public(), &store.library(true), password)?;
    store.restore()?;
    Ok(())
}

fn provision_to(
    paths: &[(PathBuf, Client)],
    public_path: &Path,
    backup_path: &Path,
    password: &str,
) -> Result<(), VaultError> {
    if paths.is_empty() || paths.len() > MAX_TEMPLATES {
        return Err(VaultError::Format);
    }
    let public = absolute(public_path)?;
    let backup = absolute(backup_path)?;
    if public == backup || backup.starts_with(&public) {
        return Err(VaultError::Format);
    }
    super::checked_parent(&public)?;
    unused(&public)?;
    unused(&backup)?;
    let mut originals = Library {
        templates: Vec::new(),
    };
    let mut sanitized = Library {
        templates: Vec::new(),
    };
    let mut sources = HashSet::new();
    for (path, client) in paths {
        let canonical = path.canonicalize().map_err(|_| VaultError::Io)?;
        if !sources.insert(canonical) {
            return Err(VaultError::Format);
        }
        let original = template::import_template(path, *client)?;
        sanitized.templates.push(template::sanitize(&original)?);
        originals.templates.push(original);
    }
    let originals = create(originals, password)?;
    let sanitized = create(sanitized, password)?;
    let backup_bytes = originals.seal()?;
    let public_bytes = sanitized.seal()?;
    super::create_private_parent(&backup)?;
    super::checked_parent(&backup)?;
    // Keep a verified encrypted original if publishing the second file fails.
    write_private(&backup, &backup_bytes, false)?;
    verify(&backup, originals.library(), password)?;
    write_private(&public, &public_bytes, false)?;
    verify(&public, sanitized.library(), password)?;
    for ((path, _), original) in paths.iter().zip(&originals.library().templates) {
        let current = Zeroizing::new(read_bounded(path)?);
        if current.as_slice() != original.content.as_bytes() {
            return Err(VaultError::Io);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::{read_bounded, unlock};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "script-hub-provision-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn managed_provision_uses_only_fixed_public_and_ignored_private_locations() {
        let directory = TestDirectory::new();
        fs::write(directory.0.join(".git"), "synthetic worktree marker").unwrap();
        fs::create_dir_all(directory.0.join("crates/processor/assets")).unwrap();
        let source = directory.0.join("fixture.surge.conf");
        let original = "[Proxy Group]\nExample = select, policy-path=https://subscription.fixture/private-path\n";
        fs::write(&source, original).unwrap();
        provision_managed(
            &[(source.clone(), Client::Surge)],
            &directory.0,
            "fixture password long",
        )
        .unwrap();
        let public = directory.0.join("crates/processor/assets/templates.vault");
        let private = directory.0.join(".cache/vaults/originals.vault");
        let originals = unlock(&read_bounded(&private).unwrap(), "fixture password long").unwrap();
        assert_eq!(originals.library().templates[0].content, original);
        let sanitized = unlock(&read_bounded(&public).unwrap(), "fixture password long").unwrap();
        assert!(
            !sanitized.library().templates[0]
                .content
                .contains("subscription.fixture")
        );
        assert_eq!(fs::read_to_string(source).unwrap(), original);
        let registry: serde_json::Value = serde_json::from_slice(
            &read_bounded(&directory.0.join(".cache/vaults/locations.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(registry["originals"], "originals.vault");
    }

    #[test]
    fn original_backup_cannot_be_created_inside_a_git_worktree() {
        let directory = TestDirectory::new();
        let repository = directory.0.join("repository");
        fs::create_dir(&repository).unwrap();
        fs::write(repository.join(".git"), "gitdir: elsewhere\n").unwrap();
        let source = directory.0.join("fixture.surge.conf");
        fs::write(&source, "[Rule]\nDOMAIN,example.test,DIRECT\n").unwrap();
        let public = directory.0.join("templates.vault");
        let backup = repository.join("originals.vault");
        let result = provision(
            &[(source.clone(), Client::Surge)],
            &public,
            &backup,
            "fixture password long",
        );
        assert_eq!(result, Err(VaultError::Format));
        assert!(!public.exists() && !backup.exists());
        assert_eq!(
            fs::read_to_string(source).unwrap(),
            "[Rule]\nDOMAIN,example.test,DIRECT\n"
        );
    }

    #[test]
    fn provisioning_recovers_exact_originals_and_only_sanitized_public_content() {
        let directory = TestDirectory::new();
        let source = directory.0.join("fixture.surge.conf");
        let original = b"[Proxy Group]\r\nExample = select, policy-path=https://subscription.fixture/private-path\r\n";
        fs::write(&source, original).unwrap();
        let public = directory.0.join("templates.vault");
        let backup = directory.0.join("private/vaults/originals.vault");

        provision(
            &[(source.clone(), Client::Surge)],
            &public,
            &backup,
            "fixture-password-exact",
        )
        .unwrap();

        let originals = unlock(&read_bounded(&backup).unwrap(), "fixture-password-exact").unwrap();
        assert_eq!(
            originals.library().templates[0].content.as_bytes(),
            original
        );
        let sanitized = unlock(&read_bounded(&public).unwrap(), "fixture-password-exact").unwrap();
        assert!(
            sanitized.library().templates[0]
                .content
                .contains("{{SUBSCRIPTION_1}}")
        );
        assert!(
            !sanitized.library().templates[0]
                .content
                .contains("private-path")
        );
        assert_eq!(fs::read(&source).unwrap(), original);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(backup.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn provisioning_refuses_existing_targets_without_changing_sources() {
        let directory = TestDirectory::new();
        let source = directory.0.join("fixture.conf");
        fs::write(&source, "[General]\ndns-server = system\n").unwrap();
        let public = directory.0.join("existing.vault");
        fs::write(&public, b"keep-existing").unwrap();
        let backup = directory.0.join("new.vault");

        assert!(
            provision(
                &[(source.clone(), Client::Shadowrocket)],
                &public,
                &backup,
                "fixture-password-exact"
            )
            .is_err()
        );

        assert_eq!(fs::read(&public).unwrap(), b"keep-existing");
        assert!(!backup.exists());
        assert_eq!(
            fs::read_to_string(source).unwrap(),
            "[General]\ndns-server = system\n"
        );
    }

    #[test]
    fn provisioning_rejects_a_destination_equal_to_a_source() {
        let directory = TestDirectory::new();
        let source = directory.0.join("fixture.conf");
        fs::write(&source, "[General]\ndns-server = system\n").unwrap();
        let backup = directory.0.join("new.vault");
        assert!(
            provision(
                &[(source.clone(), Client::Surge)],
                &source,
                &backup,
                "fixture-password-exact"
            )
            .is_err()
        );
        assert!(!backup.exists());
        assert_eq!(
            fs::read_to_string(source).unwrap(),
            "[General]\ndns-server = system\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn provisioning_refuses_symlinked_backup_directories() {
        use std::os::unix::fs::symlink;
        let directory = TestDirectory::new();
        let source = directory.0.join("fixture.conf");
        fs::write(&source, "[General]\ndns-server = system\n").unwrap();
        let outside = directory.0.join("outside");
        fs::create_dir(&outside).unwrap();
        let alias = directory.0.join("alias");
        symlink(&outside, &alias).unwrap();
        let public = directory.0.join("public.vault");
        assert!(
            provision(
                &[(source, Client::Surge)],
                &public,
                &alias.join("backup.vault"),
                "fixture-password-exact"
            )
            .is_err()
        );
        assert!(!public.exists());
        assert!(fs::read_dir(outside).unwrap().next().is_none());
    }
}
