//! Fixed project-local ciphertext storage. Records contain relative filenames, never secrets.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{VaultError, checked_parent, create_private_parent, read_bounded, write_private};

#[derive(Clone)]
pub(crate) struct ManagedStore {
    root: PathBuf,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    library: Option<String>,
    originals: Option<String>,
}

impl Default for Record {
    fn default() -> Self {
        Self {
            version: 1,
            library: None,
            originals: None,
        }
    }
}

impl ManagedStore {
    pub(crate) fn new(root: &Path) -> Result<Self, VaultError> {
        if !root.is_absolute() || root.components().any(|part| part == Component::ParentDir) {
            return Err(VaultError::Format);
        }
        checked_parent(&root.join(".vault-directory-check"))?;
        Ok(Self {
            root: root.to_owned(),
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn library(&self, originals: bool) -> PathBuf {
        self.root.join(".cache/vaults").join(if originals {
            "originals.vault"
        } else {
            "templates.vault"
        })
    }

    pub(crate) fn public(&self) -> PathBuf {
        self.root.join("crates/processor/assets/templates.vault")
    }

    fn registry(&self) -> PathBuf {
        self.root.join(".cache/vaults/locations.json")
    }

    fn available(&self) -> Result<bool, VaultError> {
        for directory in [self.root.join(".cache"), self.root.join(".cache/vaults")] {
            match fs::symlink_metadata(&directory) {
                Ok(_) => {
                    checked_parent(&directory.join(".vault-directory-check"))?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(_) => return Err(VaultError::Io),
            }
        }
        Ok(true)
    }

    pub(crate) fn restore(&self) -> Result<Option<PathBuf>, VaultError> {
        if !self.available()? {
            return Ok(None);
        }
        let mut record = self.read_record()?;
        let previous = record.clone();
        // Old backups are no longer an active library; leave their ciphertext untouched.
        let library = valid_library(&self.library(false))?.then(|| self.library(false));
        record.library = library.as_ref().map(|_| "templates.vault".to_owned());
        record.originals = None;
        self.update_record(&previous, &record)?;
        Ok(library)
    }

    fn read_record(&self) -> Result<Record, VaultError> {
        let registry = self.registry();
        match fs::symlink_metadata(&registry) {
            Ok(_) => {
                let bytes = super::io::read_limited(&registry, 4096)?;
                let record: Record =
                    serde_json::from_slice(&bytes).map_err(|_| VaultError::Format)?;
                if record.version != 1 {
                    return Err(VaultError::Format);
                }
                Ok(record)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Record::default()),
            Err(_) => Err(VaultError::Io),
        }
    }

    fn update_record(&self, previous: &Record, record: &Record) -> Result<(), VaultError> {
        if record != previous {
            let bytes = serde_json::to_vec(record).map_err(|_| VaultError::Format)?;
            write_private(&self.registry(), &bytes, true)?;
        }
        Ok(())
    }

    /// Removes stale names without reading ciphertext or changing an active session's scope.
    pub(crate) fn prune_missing(&self) -> Result<Option<PathBuf>, VaultError> {
        if !self.available()? {
            return Ok(None);
        }
        let mut record = self.read_record()?;
        let previous = record.clone();
        if record.library.as_deref() != Some("templates.vault")
            || !regular_file(&self.library(false))?
        {
            record.library = None;
        }
        record.originals = None;
        self.update_record(&previous, &record)?;
        Ok(record.library.map(|_| self.library(false)))
    }

    /// Publishes the private working library, never a retired backup.
    pub(crate) fn publish(&self, bytes: &[u8], overwrite: bool) -> Result<PathBuf, VaultError> {
        super::crypto::validate_envelope(bytes)?;
        let destination = self.library(false);
        create_private_parent(&destination)?;
        self.restore()?; // Registry failures must be visible before publishing more ciphertext.
        write_private(&destination, bytes, overwrite)?;
        if read_bounded(&destination)? != bytes {
            return Err(VaultError::Io);
        }
        self.restore()?;
        Ok(destination)
    }

    pub(crate) fn import_authenticated(
        &self,
        source: &Path,
        bytes: &[u8],
    ) -> Result<PathBuf, VaultError> {
        let destination = self.library(false);
        if destination.canonicalize().ok().as_ref()
            == Some(&source.canonicalize().map_err(|_| VaultError::Io)?)
        {
            self.restore()?;
            if read_bounded(&destination)? != bytes {
                return Err(VaultError::Io);
            }
            return Ok(destination);
        }
        self.publish(bytes, false)
    }
}

fn valid_library(path: &Path) -> Result<bool, VaultError> {
    if !regular_file(path)? {
        return Ok(false);
    }
    match read_bounded(path) {
        Ok(bytes) => Ok(super::crypto::validate_envelope(&bytes).is_ok()),
        Err(VaultError::TooLarge) => Ok(false),
        Err(error) => Err(error),
    }
}

fn regular_file(path: &Path) -> Result<bool, VaultError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && !super::is_link(&metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(VaultError::Io),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::{Library, create, unlock};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "managed-vault-store-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }

        fn bytes(&self) -> Vec<u8> {
            create(
                Library {
                    templates: Vec::new(),
                },
                "synthetic fixture password",
            )
            .unwrap()
            .seal()
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn authenticated_import_is_verified_no_replace_and_keeps_the_source() {
        let fixture = Fixture::new();
        let store = ManagedStore::new(&fixture.0).unwrap();
        let source = fixture.0.join("external-original.vault");
        let bytes = fixture.bytes();
        write_private(&source, &bytes, false).unwrap();
        drop(unlock(&bytes, "synthetic fixture password").unwrap());
        let copied = store.import_authenticated(&source, &bytes).unwrap();
        assert_eq!(read_bounded(&source).unwrap(), bytes);
        assert_eq!(read_bounded(&copied).unwrap(), bytes);
        let record: serde_json::Value =
            serde_json::from_slice(&read_bounded(&store.registry()).unwrap()).unwrap();
        assert_eq!(record["library"], "templates.vault");
        assert!(record["originals"].is_null());
        let replacement = fixture.bytes();
        assert_eq!(
            store.publish(&replacement, false),
            Err(VaultError::AlreadyExists)
        );
        assert_eq!(read_bounded(&copied).unwrap(), bytes);
    }

    #[test]
    fn retired_originals_are_not_restored_or_rewritten() {
        let fixture = Fixture::new();
        let store = ManagedStore::new(&fixture.0).unwrap();
        create_private_parent(&store.registry()).unwrap();
        write_private(
            &store.registry(),
            br#"{"version":1,"library":null,"originals":"originals.vault"}"#,
            false,
        )
        .unwrap();
        let bytes = b"opaque retired backup";
        write_private(&store.library(true), bytes, false).unwrap();
        assert!(store.restore().unwrap().is_none());
        assert_eq!(read_bounded(&store.library(true)).unwrap(), bytes);
        assert!(store.read_record().unwrap().originals.is_none());
        assert!(!store.library(false).exists());
    }

    #[test]
    fn malformed_registry_fails_closed_before_publishing_ciphertext() {
        let fixture = Fixture::new();
        let store = ManagedStore::new(&fixture.0).unwrap();
        create_private_parent(&store.registry()).unwrap();
        write_private(&store.registry(), b"not a registry", false).unwrap();
        assert_eq!(
            store.publish(&fixture.bytes(), false),
            Err(VaultError::Format)
        );
        assert!(!store.library(false).exists());
        assert_eq!(read_bounded(&store.registry()).unwrap(), b"not a registry");
    }

    #[test]
    fn registry_reader_enforces_its_small_limit_before_decoding() {
        let fixture = Fixture::new();
        let store = ManagedStore::new(&fixture.0).unwrap();
        create_private_parent(&store.registry()).unwrap();
        let exact = vec![b' '; 4096];
        write_private(&store.registry(), &exact, false).unwrap();
        assert_eq!(
            super::super::io::read_limited(&store.registry(), 4096).unwrap(),
            exact
        );
        let oversized = vec![b' '; 4097];
        write_private(&store.registry(), &oversized, true).unwrap();
        assert_eq!(
            super::super::io::read_limited(&store.registry(), 4096),
            Err(VaultError::TooLarge)
        );
        assert!(matches!(store.restore(), Err(VaultError::TooLarge)));
        assert_eq!(read_bounded(&store.registry()).unwrap(), oversized);
        assert!(!store.library(false).exists() && !store.library(true).exists());
    }

    #[test]
    fn damaged_ciphertext_is_preserved_while_its_record_is_cleared() {
        let fixture = Fixture::new();
        let store = ManagedStore::new(&fixture.0).unwrap();
        create_private_parent(&store.registry()).unwrap();
        write_private(
            &store.registry(),
            br#"{"version":1,"library":"templates.vault","originals":null}"#,
            false,
        )
        .unwrap();
        write_private(&store.library(false), b"damaged encrypted fixture", false).unwrap();
        assert!(store.restore().unwrap().is_none());
        assert_eq!(
            read_bounded(&store.library(false)).unwrap(),
            b"damaged encrypted fixture"
        );
        let record: serde_json::Value =
            serde_json::from_slice(&read_bounded(&store.registry()).unwrap()).unwrap();
        assert!(record["library"].is_null());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_storage_and_registry_are_never_followed_or_removed() {
        let fixture = Fixture::new();
        let external = Fixture::new();
        let store = ManagedStore::new(&fixture.0).unwrap();
        std::os::unix::fs::symlink(&external.0, fixture.0.join(".cache")).unwrap();
        assert!(store.restore().is_err());
        assert!(store.publish(&fixture.bytes(), false).is_err());
        assert!(
            fs::symlink_metadata(fixture.0.join(".cache"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(fs::read_dir(&external.0).unwrap().next().is_none());
        fs::remove_file(fixture.0.join(".cache")).unwrap();
        create_private_parent(&store.registry()).unwrap();
        let original = external.0.join("kept-record.json");
        fs::write(&original, b"kept fixture").unwrap();
        std::os::unix::fs::symlink(&original, store.registry()).unwrap();
        assert!(store.restore().is_err());
        assert_eq!(fs::read(original).unwrap(), b"kept fixture");
        assert!(
            fs::symlink_metadata(store.registry())
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}
