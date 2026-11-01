use std::io::{self, Write};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use zeroize::Zeroizing;

use super::{
    Library, MAX_PASSWORD_BYTES, MAX_PLAINTEXT_BYTES, MIN_PASSWORD_CHARS, VaultError,
    validate_library,
};

const MAGIC: &[u8; 8] = b"SHVAULT\0";
const VERSION: u8 = 1;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const HEADER_BYTES: usize = 8 + 1 + SALT_BYTES + NONCE_BYTES;
const TAG_BYTES: usize = 16;
pub(super) const MAX_FILE_BYTES: usize = HEADER_BYTES + MAX_PLAINTEXT_BYTES + TAG_BYTES;

/// An explicitly unlocked library. Dropping it erases its owned plaintext and key buffers.
pub struct UnlockedVault {
    library: Library,
    key: Zeroizing<[u8; 32]>,
    salt: [u8; SALT_BYTES],
}

impl UnlockedVault {
    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn library_mut(&mut self) -> &mut Library {
        &mut self.library
    }

    #[cfg(all(feature = "gui", any(target_os = "windows", target_os = "macos")))]
    pub(crate) fn validate_for_storage(&self) -> Result<(), VaultError> {
        validate_library(&self.library)?;
        drop(encode_library(&self.library)?);
        Ok(())
    }

    /// Authenticates the full library and format header with a fresh nonce for every save.
    pub fn seal(&self) -> Result<Vec<u8>, VaultError> {
        validate_library(&self.library)?;
        let plaintext = encode_library(&self.library)?;
        let mut nonce = [0u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| VaultError::Randomness)?;
        let mut header = [0u8; HEADER_BYTES];
        header[..8].copy_from_slice(MAGIC);
        header[8] = VERSION;
        header[9..25].copy_from_slice(&self.salt);
        header[25..].copy_from_slice(&nonce);
        let cipher =
            XChaCha20Poly1305::new_from_slice(self.key.as_ref()).map_err(|_| VaultError::Format)?;
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext.as_slice(),
                    aad: &header,
                },
            )
            .map_err(|_| VaultError::Format)?;
        let mut result = Vec::with_capacity(HEADER_BYTES + ciphertext.len());
        result.extend_from_slice(&header);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }
}

pub fn create(library: Library, password: &str) -> Result<UnlockedVault, VaultError> {
    validate_password(password)?;
    validate_library(&library)?;
    drop(encode_library(&library)?);
    let mut salt = [0u8; SALT_BYTES];
    getrandom::fill(&mut salt).map_err(|_| VaultError::Randomness)?;
    let key = derive_key(password, &salt)?;
    Ok(UnlockedVault { library, key, salt })
}

pub(crate) fn validate_envelope(ciphertext: &[u8]) -> Result<(), VaultError> {
    if ciphertext.len() > MAX_FILE_BYTES {
        return Err(VaultError::TooLarge);
    }
    if ciphertext.len() < HEADER_BYTES + TAG_BYTES
        || &ciphertext[..8] != MAGIC
        || ciphertext[8] != VERSION
    {
        return Err(VaultError::Format);
    }
    Ok(())
}

pub fn unlock(ciphertext: &[u8], password: &str) -> Result<UnlockedVault, VaultError> {
    validate_envelope(ciphertext)?;
    validate_password(password)?;
    let salt: [u8; SALT_BYTES] = ciphertext[9..25]
        .try_into()
        .map_err(|_| VaultError::Format)?;
    let nonce: [u8; NONCE_BYTES] = ciphertext[25..HEADER_BYTES]
        .try_into()
        .map_err(|_| VaultError::Format)?;
    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| VaultError::Format)?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &ciphertext[HEADER_BYTES..],
                    aad: &ciphertext[..HEADER_BYTES],
                },
            )
            .map_err(|_| VaultError::Authentication)?,
    );
    let library: Library = serde_json::from_slice(&plaintext).map_err(|_| VaultError::Format)?;
    validate_library(&library)?;
    Ok(UnlockedVault { library, key, salt })
}

fn validate_password(password: &str) -> Result<(), VaultError> {
    if password.len() > MAX_PASSWORD_BYTES || password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(VaultError::Password);
    }
    Ok(())
}

fn derive_key(password: &str, salt: &[u8; SALT_BYTES]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let mut key = Zeroizing::new([0u8; 32]);
    // Fixed single-lane costs bound hostile imports and avoid parallel KDF memory pressure.
    let params = Params::new(65_536, 3, 1, Some(32)).map_err(|_| VaultError::Format)?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut *key)
        .map_err(|_| VaultError::Format)?;
    Ok(key)
}

struct BoundedPlaintext(Zeroizing<Vec<u8>>);

fn encode_library(library: &Library) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    let mut plaintext = BoundedPlaintext(Zeroizing::new(Vec::new()));
    plaintext
        .0
        .try_reserve_exact(MAX_PLAINTEXT_BYTES)
        .map_err(|_| VaultError::TooLarge)?;
    serde_json::to_writer(&mut plaintext, library).map_err(|_| VaultError::TooLarge)?;
    Ok(plaintext.0)
}

impl Write for BoundedPlaintext {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_PLAINTEXT_BYTES - self.0.len() {
            return Err(io::Error::other("Template library limit exceeded"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
