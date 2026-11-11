//! Password-encrypted template libraries. Plaintext exists only while explicitly unlocked.

mod crypto;
mod io;
pub(crate) mod managed;
pub mod provision;
pub mod template;

use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub(crate) use crypto::Envelope;
pub use crypto::{UnlockedVault, create, unlock};
pub(crate) use io::{checked_parent, create_private_parent, is_link};
pub use io::{read_bounded, write_private};

pub const MAX_PLAINTEXT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TEMPLATE_BYTES: usize = 1024 * 1024;
pub const MAX_TEMPLATES: usize = 64;
pub const MAX_PASSWORD_BYTES: usize = 1024;
pub const MIN_PASSWORD_CHARS: usize = 12;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Client {
    Surge,
    Shadowrocket,
    FlClash,
    Other,
}

#[derive(Deserialize, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub name: String,
    #[zeroize(skip)]
    pub client: Client,
    pub content: String,
}

#[derive(Deserialize, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub templates: Vec<Template>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultError {
    Password,
    Format,
    TooLarge,
    Authentication,
    Randomness,
    Io,
    AlreadyExists,
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Password => "Use a password of at least 12 characters and at most 1024 bytes.",
            Self::Format => "The template library format or content is invalid.",
            Self::TooLarge => "The template library exceeds its supported size limits.",
            Self::Authentication => "The password is incorrect or the library is damaged.",
            Self::Randomness => "Secure system randomness is unavailable.",
            Self::Io => "The file could not be safely read or written.",
            Self::AlreadyExists => "A file already exists at the selected destination.",
        })
    }
}

impl std::error::Error for VaultError {}

fn validate_library(library: &Library) -> Result<(), VaultError> {
    if library.templates.len() > MAX_TEMPLATES {
        return Err(VaultError::TooLarge);
    }
    let mut total = 0usize;
    for template in &library.templates {
        if template.name.trim().is_empty()
            || template.content.is_empty()
            || template.name.contains(['\0', '\r', '\n'])
            || template.content.contains('\0')
        {
            return Err(VaultError::Format);
        }
        if template.name.len() > 256 || template.content.len() > MAX_TEMPLATE_BYTES {
            return Err(VaultError::TooLarge);
        }
        total = total
            .checked_add(template.name.len())
            .and_then(|sum| sum.checked_add(template.content.len()))
            .ok_or(VaultError::TooLarge)?;
        if total > MAX_PLAINTEXT_BYTES {
            return Err(VaultError::TooLarge);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
