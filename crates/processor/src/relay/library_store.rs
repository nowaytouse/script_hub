use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::library::Workspace;
use crate::vault::{self, Envelope, VaultError};

const MAGIC: [u8; 8] = *b"SHSUBS\0\0";

#[derive(Debug)]
pub enum StoreError {
    Vault(VaultError),
    InvalidWorkspace,
    Conflict,
    Location,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Vault(VaultError::Password) => {
                "Use a password of at least 12 characters and at most 1024 bytes."
            }
            Self::Vault(VaultError::Authentication) => {
                "The password is incorrect or the subscription library is damaged."
            }
            Self::Vault(VaultError::AlreadyExists) => "A subscription library already exists.",
            Self::Vault(VaultError::TooLarge) => {
                "The subscription library exceeds its supported size limit."
            }
            Self::Vault(_) => "The subscription library could not be safely read or written.",
            Self::InvalidWorkspace => "The subscription library content is invalid.",
            Self::Conflict => "The subscription library changed on disk; reopen it before saving.",
            Self::Location => "The application data location is unavailable.",
        })
    }
}

impl std::error::Error for StoreError {}

impl From<VaultError> for StoreError {
    fn from(error: VaultError) -> Self {
        Self::Vault(error)
    }
}

pub struct Store {
    path: PathBuf,
    envelope: Envelope,
    expected: Mutex<Vec<u8>>,
}

impl Store {
    pub fn path() -> Result<PathBuf, StoreError> {
        let preferences =
            crate::runtime::settings::preferences_path().map_err(|_| StoreError::Location)?;
        Ok(preferences
            .parent()
            .ok_or(StoreError::Location)?
            .join("subscriptions.vault"))
    }

    pub fn exists() -> Result<bool, StoreError> {
        exists_at(&Self::path()?)
    }

    pub fn create(workspace: &Workspace, password: &str) -> Result<Self, StoreError> {
        Self::create_at(Self::path()?, workspace, password)
    }

    pub fn open(password: &str) -> Result<(Self, Workspace), StoreError> {
        Self::open_at(Self::path()?, password)
    }

    fn create_at(path: PathBuf, workspace: &Workspace, password: &str) -> Result<Self, StoreError> {
        let plaintext = workspace
            .encode()
            .map_err(|_| StoreError::InvalidWorkspace)?;
        let envelope = Envelope::create(password, MAGIC)?;
        let bytes = envelope.seal(&plaintext)?;
        vault::create_private_parent(&path)?;
        let _lock = lock_at(&path)?;
        vault::write_private(&path, &bytes, false)?;
        Ok(Self {
            path,
            envelope,
            expected: Mutex::new(bytes),
        })
    }

    fn open_at(path: PathBuf, password: &str) -> Result<(Self, Workspace), StoreError> {
        let bytes = vault::read_bounded(&path)?;
        let (envelope, plaintext) = Envelope::unlock(&bytes, password, MAGIC)?;
        let workspace = Workspace::decode(&plaintext).map_err(|_| StoreError::InvalidWorkspace)?;
        Ok((
            Self {
                path,
                envelope,
                expected: Mutex::new(bytes),
            },
            workspace,
        ))
    }

    pub fn save(&self, workspace: &Workspace) -> Result<(), StoreError> {
        let plaintext = workspace
            .encode()
            .map_err(|_| StoreError::InvalidWorkspace)?;
        let bytes = self.envelope.seal(&plaintext)?;
        let mut expected = self.expected.lock().map_err(|_| StoreError::Conflict)?;
        let _lock = lock_at(&self.path)?;
        if vault::read_bounded(&self.path)? != *expected {
            return Err(StoreError::Conflict);
        }
        vault::write_private(&self.path, &bytes, true)?;
        *expected = bytes;
        Ok(())
    }
}

fn lock_at(path: &Path) -> Result<std::fs::File, StoreError> {
    let parent = vault::checked_parent(path)?;
    let lock = parent.join("subscriptions.lock");
    match vault::write_private(&lock, &[], false) {
        Ok(()) | Err(VaultError::AlreadyExists) => {}
        Err(error) => return Err(error.into()),
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    let file = options.open(&lock).map_err(|_| VaultError::Io)?;
    let metadata = file.metadata().map_err(|_| VaultError::Io)?;
    if !metadata.is_file() || vault::is_link(&metadata) {
        return Err(VaultError::Io.into());
    }
    file.try_lock().map_err(|_| StoreError::Conflict)?;
    Ok(file)
}

fn exists_at(path: &Path) -> Result<bool, StoreError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !vault::is_link(&metadata) => {
            vault::checked_parent(path)?;
            Ok(true)
        }
        Ok(_) => Err(VaultError::Io.into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::verify_missing_directory(path).map_err(|_| StoreError::Location)?;
            Ok(false)
        }
        Err(_) => Err(VaultError::Io.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_roundtrip_and_stale_writer_conflict() {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "script-hub-store-{:x}",
            u128::from_ne_bytes(random)
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("subscriptions.vault");
        let workspace = Workspace::default();
        let first = Store::create_at(path.clone(), &workspace, "long enough password").unwrap();
        assert!(Store::create_at(path.clone(), &workspace, "long enough password").is_err());
        let (second, _) = Store::open_at(path.clone(), "long enough password").unwrap();
        first.save(&workspace).unwrap();
        let current = std::fs::read(&path).unwrap();
        assert!(matches!(second.save(&workspace), Err(StoreError::Conflict)));
        assert_eq!(std::fs::read(&path).unwrap(), current);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(directory.join("subscriptions.lock")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
