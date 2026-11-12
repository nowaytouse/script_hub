use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use super::VaultError;
use super::crypto::MAX_FILE_BYTES;

/// Reads only a bounded regular file, refusing symlinks at the selected file.
pub fn read_bounded(path: &Path) -> Result<Vec<u8>, VaultError> {
    read_limited(path, MAX_FILE_BYTES)
}

pub(crate) fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, VaultError> {
    let parent = checked_parent(path)?;
    let path = parent.join(path.file_name().ok_or(VaultError::Io)?);
    let metadata = fs::symlink_metadata(&path).map_err(|_| VaultError::Io)?;
    if !metadata.is_file() || is_link(&metadata) {
        return Err(VaultError::Io);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(&path).map_err(|_| VaultError::Io)?;
    let metadata = file.metadata().map_err(|_| VaultError::Io)?;
    if !metadata.is_file() || is_link(&metadata) {
        return Err(VaultError::Io);
    }
    if metadata.len() > limit as u64 {
        return Err(VaultError::TooLarge);
    }
    let mut bytes = Zeroizing::new(vec![0u8; metadata.len() as usize]);
    file.read_exact(&mut bytes).map_err(|_| VaultError::Io)?;
    let mut extra = Zeroizing::new([0u8; 1]);
    if file.read(&mut *extra).map_err(|_| VaultError::Io)? != 0 {
        return Err(VaultError::Io);
    }
    let after = file.metadata().map_err(|_| VaultError::Io)?;
    if after.len() != metadata.len()
        || after.modified().map_err(|_| VaultError::Io)?
            != metadata.modified().map_err(|_| VaultError::Io)?
    {
        return Err(VaultError::Io);
    }
    Ok(std::mem::take(&mut *bytes))
}

/// Publishes exact bytes through a private staging file; the parent directory must already exist.
pub fn write_private(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), VaultError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(VaultError::TooLarge);
    }
    let parent = checked_parent(path)?;
    let path = parent.join(path.file_name().ok_or(VaultError::Io)?);
    check_target(&path, overwrite)?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| VaultError::Randomness)?;
    let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let temporary = parent.join(format!(".shvault-{suffix}.writing"));
    #[cfg(windows)]
    {
        write_private_windows(&path, &temporary, bytes, overwrite)
    }
    #[cfg(not(windows))]
    {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|_| VaultError::Io)?;
        let result = (|| {
            file.write_all(bytes).map_err(|_| VaultError::Io)?;
            file.sync_all().map_err(|_| VaultError::Io)?;
            drop(file);
            checked_parent(&path)?;
            check_target(&path, overwrite)?;
            if overwrite {
                fs::rename(&temporary, &path).map_err(|_| VaultError::Io)?;
            } else {
                // Fail closed on filesystems without hard links; never emulate no-replace with a racy rename.
                fs::hard_link(&temporary, &path).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        VaultError::AlreadyExists
                    } else {
                        VaultError::Io
                    }
                })?;
                fs::remove_file(&temporary).map_err(|_| VaultError::Io)?;
            }
            #[cfg(unix)]
            fs::File::open(&parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| VaultError::Io)?;
            Ok(())
        })();
        if result.is_err()
            && let Err(error) = fs::remove_file(&temporary)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(VaultError::Io);
        }
        result
    }
}

#[cfg(windows)]
fn write_private_windows(
    path: &Path,
    directory: &Path,
    bytes: &[u8],
    overwrite: bool,
) -> Result<(), VaultError> {
    let identity = windows_system_command("whoami.exe")?
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .map_err(|_| VaultError::Io)?;
    if !identity.status.success() {
        return Err(VaultError::Io);
    }
    let sid = parse_whoami_sid(&identity.stdout)?;
    fs::create_dir(directory).map_err(|_| VaultError::Io)?;
    let temporary = directory.join("payload");
    let mut created = false;
    let result = (|| {
        // No payload file exists until the empty, same-volume directory has a user-only DACL.
        // Changing an already-created shared file's ACL would not revoke earlier open handles.
        windows_icacls(directory, &["/reset", "/q"])?;
        let grant = format!("*{sid}:(OI)(CI)F");
        windows_icacls(
            directory,
            &["/inheritancelevel:r", "/grant:r", &grant, "/q"],
        )?;
        checked_parent(&temporary)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| VaultError::Io)?;
        created = true;
        // Copy the sole inherited user ACE into a protected DACL while the file is still empty.
        // Same-volume publication then preserves it independently of destination inheritance.
        windows_icacls(&temporary, &["/inheritancelevel:d", "/q"])?;
        file.write_all(bytes).map_err(|_| VaultError::Io)?;
        file.sync_all().map_err(|_| VaultError::Io)?;
        drop(file);
        checked_parent(path)?;
        check_target(path, overwrite)?;
        if overwrite {
            fs::rename(&temporary, path).map_err(|_| VaultError::Io)?;
        } else {
            // A hard link preserves the source file's DACL and cannot replace an existing target.
            fs::hard_link(&temporary, path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    VaultError::AlreadyExists
                } else {
                    VaultError::Io
                }
            })?;
        }
        Ok(())
    })();
    if created
        && let Err(error) = fs::remove_file(&temporary)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(VaultError::Io);
    }
    // Fail visibly rather than hiding a leftover private staging file or directory.
    fs::remove_dir(directory).map_err(|_| VaultError::Io)?;
    result
}

#[cfg(windows)]
fn windows_system_command(name: &str) -> Result<std::process::Command, VaultError> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let root = PathBuf::from(std::env::var_os("SystemRoot").ok_or(VaultError::Io)?);
    if !root.is_absolute() {
        return Err(VaultError::Io);
    }
    let mut command = Command::new(root.join("System32").join(name));
    command.stdin(Stdio::null()).creation_flags(0x0800_0000);
    Ok(command)
}

#[cfg(windows)]
fn windows_icacls(path: &Path, arguments: &[&str]) -> Result<(), VaultError> {
    let output = windows_system_command("icacls.exe")?
        .arg(path)
        .args(arguments)
        .output()
        .map_err(|_| VaultError::Io)?;
    // Never expose captured diagnostics: they can contain private paths and account names.
    if output.status.success() {
        Ok(())
    } else {
        Err(VaultError::Io)
    }
}

fn check_target(path: &Path, overwrite: bool) -> Result<(), VaultError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || is_link(&metadata) => Err(VaultError::Io),
        Ok(_) if !overwrite => Err(VaultError::AlreadyExists),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(VaultError::Io),
    }
}

pub(crate) fn checked_parent(path: &Path) -> Result<PathBuf, VaultError> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let absolute = if parent.is_absolute() {
        parent.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|_| VaultError::Io)?
            .join(parent)
    };
    for ancestor in absolute.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| VaultError::Io)?;
        if !metadata.is_dir() || is_link(&metadata) {
            return Err(VaultError::Io);
        }
    }
    Ok(absolute)
}

pub(crate) fn create_private_parent(path: &Path) -> Result<(), VaultError> {
    let mut pending = Vec::new();
    let mut existing = path.parent().ok_or(VaultError::Format)?;
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                pending.push(existing.to_owned());
                existing = existing.parent().ok_or(VaultError::Io)?;
            }
            Err(_) => return Err(VaultError::Io),
        }
    }
    checked_parent(&existing.join(".vault-directory-check"))?;
    for directory in pending.iter().rev() {
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(directory).map_err(|_| VaultError::Io)?;
        checked_parent(&directory.join(".vault-directory-check"))?;
    }
    Ok(())
}

pub(crate) fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(any(windows, test))]
fn parse_whoami_sid(output: &[u8]) -> Result<String, VaultError> {
    if output.len() > 4096 {
        return Err(VaultError::Io);
    }
    let row = output.trim_ascii();
    if row.iter().any(|byte| matches!(*byte, b'\r' | b'\n' | 0)) {
        return Err(VaultError::Io);
    }
    let separator = row
        .iter()
        .rposition(|byte| *byte == b',')
        .ok_or(VaultError::Io)?;
    let account = &row[..separator];
    if account.len() < 2 || account.first() != Some(&b'"') || account.last() != Some(&b'"') {
        return Err(VaultError::Io);
    }
    let mut index = 1;
    while index < account.len() - 1 {
        if account[index] == b'"' {
            if index + 1 >= account.len() - 1 || account[index + 1] != b'"' {
                return Err(VaultError::Io);
            }
            index += 1;
        }
        index += 1;
    }
    let field = &row[separator + 1..];
    if field.len() < 2 || field.first() != Some(&b'"') || field.last() != Some(&b'"') {
        return Err(VaultError::Io);
    }
    let sid = std::str::from_utf8(&field[1..field.len() - 1]).map_err(|_| VaultError::Io)?;
    let parts: Vec<_> = sid.split('-').collect();
    if !(4..=18).contains(&parts.len())
        || parts[0] != "S"
        || parts[1] != "1"
        || parts[2..]
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        || parts[2].parse::<u64>().map_err(|_| VaultError::Io)? >= 1 << 48
        || parts[3..].iter().any(|part| part.parse::<u32>().is_err())
    {
        return Err(VaultError::Io);
    }
    Ok(sid.to_owned())
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn windows_identity_accepts_only_one_numeric_sid_from_csv() {
        assert_eq!(
            parse_whoami_sid(b"\"fixture\\user\",\"S-1-5-21-10-20-30-1001\"\r\n"),
            Ok("S-1-5-21-10-20-30-1001".to_owned())
        );
        // Usernames can use a non-UTF8 OEM code page; only the SID is interpreted.
        assert_eq!(
            parse_whoami_sid(b"\"fixture\\\xff\",\"S-1-12-1-10-20-30-40\"\r\n"),
            Ok("S-1-12-1-10-20-30-40".to_owned())
        );
        for invalid in [
            b"".as_slice(),
            b"\"fixture\",\"S-1-5-18\"\r\n\"other\",\"S-1-5-19\"",
            b"\"fixture\",\"S-1-5-18:(OI)(CI)F\"",
            b"\"fixture\",\"S-1-5-4294967296\"",
            b"\"fixture\",\"S-1-281474976710656-18\"",
            b"\"fixture\",\"S-2-5-18\"",
            b"\"fixture\",\"S-1-5\"",
            b"\"fixture\",\"S-1-5--18\"",
            b"\"fixture\",\"S-1-5-18\",\"extra\"",
            b"fixture,S-1-5-18",
        ] {
            assert_eq!(parse_whoami_sid(invalid), Err(VaultError::Io));
        }
    }
}
