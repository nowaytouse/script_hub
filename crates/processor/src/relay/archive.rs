//! Classic age X25519 archives only; passphrases, plugins, SSH and hybrid keys are unsupported.
use std::io::Read;

use age::{Decryptor, armor::ArmoredReader, x25519};
use zeroize::Zeroizing;

const MAX_PLAINTEXT: usize = 4 * 1024 * 1024;
const MAX_CIPHERTEXT: usize = 8 * 1024 * 1024;
// Limit untrusted decoded headers before age's incremental header parser runs.
const MAX_HEADER: usize = 8 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArchiveError {
    TooLarge,
    InvalidKey,
    Unsupported,
    InvalidArchive,
    EncryptionFailed,
}

pub(crate) fn detect(input: &[u8]) -> bool {
    input.starts_with(b"age-encryption.org/v1\n")
        || input.starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----")
}

pub(crate) fn encrypt(input: &[u8], recipient: &str) -> Result<Zeroizing<Vec<u8>>, ArchiveError> {
    if input.len() > MAX_PLAINTEXT {
        return Err(ArchiveError::TooLarge);
    }
    let recipient: x25519::Recipient = recipient.parse().map_err(|_| ArchiveError::InvalidKey)?;
    let ciphertext =
        age::encrypt_and_armor(&recipient, input).map_err(|_| ArchiveError::EncryptionFailed)?;
    if ciphertext.len() > MAX_CIPHERTEXT {
        return Err(ArchiveError::TooLarge);
    }
    Ok(Zeroizing::new(ciphertext.into_bytes()))
}

pub(crate) fn decrypt(input: &[u8], identity: &str) -> Result<Zeroizing<Vec<u8>>, ArchiveError> {
    if input.len() > MAX_CIPHERTEXT {
        return Err(ArchiveError::TooLarge);
    }
    let identity: x25519::Identity = identity.parse().map_err(|_| ArchiveError::InvalidKey)?;
    let mut prefix = Vec::new();
    ArmoredReader::new(input)
        .take((MAX_HEADER + 1) as u64)
        .read_to_end(&mut prefix)
        .map_err(|_| ArchiveError::InvalidArchive)?;
    let mut end = 0;
    let found = prefix.split_inclusive(|byte| *byte == b'\n').any(|line| {
        end += line.len();
        line.starts_with(b"--- ") && line.ends_with(b"\n") && end <= MAX_HEADER
    });
    if !found {
        return Err(if prefix.len() > MAX_HEADER {
            ArchiveError::TooLarge
        } else {
            ArchiveError::InvalidArchive
        });
    }
    let decryptor =
        Decryptor::new(ArmoredReader::new(input)).map_err(|_| ArchiveError::InvalidArchive)?;
    if decryptor.is_scrypt() {
        return Err(ArchiveError::Unsupported);
    }
    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| ArchiveError::InvalidArchive)?;
    let mut plaintext = Zeroizing::new(Vec::new());
    reader
        .take((MAX_PLAINTEXT + 1) as u64)
        .read_to_end(&mut plaintext)
        .map_err(|_| ArchiveError::InvalidArchive)?;
    if plaintext.len() > MAX_PLAINTEXT {
        return Err(ArchiveError::TooLarge);
    }
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::secrecy::ExposeSecret;
    use std::io::Write;

    #[test]
    fn roundtrip_and_reject_damage() {
        let key = x25519::Identity::generate();
        let secret = key.to_string();
        let ciphertext = encrypt(b"eyJ0ZXN0Ijp0cnVlfQ==", &key.to_public().to_string()).unwrap();
        assert!(detect(&ciphertext));
        assert_eq!(
            &**decrypt(&ciphertext, secret.expose_secret()).unwrap(),
            b"eyJ0ZXN0Ijp0cnVlfQ=="
        );
        let other = x25519::Identity::generate().to_string();
        assert!(decrypt(&ciphertext, other.expose_secret()).is_err());
        assert!(decrypt(&ciphertext[..ciphertext.len() / 2], secret.expose_secret()).is_err());
        assert!(decrypt(&ciphertext[..ciphertext.len() - 10], secret.expose_secret()).is_err());
        let mut binary = age::encrypt(&key.to_public(), b"payload").unwrap();
        assert!(detect(&binary));
        assert_eq!(
            &**decrypt(&binary, secret.expose_secret()).unwrap(),
            b"payload"
        );
        let last = binary.len() - 1;
        binary[last] ^= 1;
        assert!(decrypt(&binary, secret.expose_secret()).is_err());
    }

    #[test]
    fn bounds_and_unsupported_inputs() {
        let key = x25519::Identity::generate();
        let secret = key.to_string();
        assert_eq!(
            encrypt(&vec![0; MAX_PLAINTEXT + 1], &key.to_public().to_string()).unwrap_err(),
            ArchiveError::TooLarge
        );
        assert_eq!(
            decrypt(&vec![0; MAX_CIPHERTEXT + 1], secret.expose_secret()).unwrap_err(),
            ArchiveError::TooLarge
        );
        assert_eq!(
            decrypt(b"not an archive", "AGE-SECRET-KEY-PQ-1invalid").unwrap_err(),
            ArchiveError::InvalidKey
        );
        let large = age::encrypt(&key.to_public(), &vec![0; MAX_PLAINTEXT + 1]).unwrap();
        assert_eq!(
            decrypt(&large, secret.expose_secret()).unwrap_err(),
            ArchiveError::TooLarge
        );
    }

    #[test]
    fn rejects_passphrase_before_work_factor() {
        let key = x25519::Identity::generate().to_string();
        let passphrase = age::scrypt::Recipient::new("test-only".to_owned().into());
        let ciphertext = age::encrypt(&passphrase, b"test").unwrap();
        let end = ciphertext.iter().position(|&b| b == b'\n').unwrap() + 1;
        let line_end = ciphertext[end..].iter().position(|&b| b == b'\n').unwrap() + end;
        let line = std::str::from_utf8(&ciphertext[end..line_end]).unwrap();
        let (stanza, _) = line.rsplit_once(' ').unwrap();
        let mut excessive = ciphertext[..end].to_vec();
        excessive.extend_from_slice(format!("{stanza} 63\n").as_bytes());
        excessive.extend_from_slice(&ciphertext[line_end + 1..]);
        assert_eq!(
            decrypt(&excessive, key.expose_secret()).unwrap_err(),
            ArchiveError::Unsupported
        );
    }

    #[test]
    fn rejects_oversized_binary_and_armored_headers() {
        let key = x25519::Identity::generate();
        let secret = key.to_string();
        let mut binary = b"age-encryption.org/v1\n".to_vec();
        for _ in 0..200 {
            binary.extend_from_slice(b"-> unknown\nYWJj\n");
        }
        binary.extend_from_slice(&vec![b'A'; MAX_HEADER]);
        binary.extend_from_slice(b"\n--- AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n");
        assert_eq!(
            decrypt(&binary, secret.expose_secret()).unwrap_err(),
            ArchiveError::TooLarge
        );
        let mut writer =
            age::armor::ArmoredWriter::wrap_output(Vec::new(), age::armor::Format::AsciiArmor)
                .unwrap();
        writer.write_all(&binary).unwrap();
        let armored = writer.finish().unwrap();
        assert_eq!(
            decrypt(&armored, secret.expose_secret()).unwrap_err(),
            ArchiveError::TooLarge
        );
    }
}
