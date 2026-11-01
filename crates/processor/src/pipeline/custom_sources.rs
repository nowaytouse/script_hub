use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MANIFEST_VERSION: u32 = 1;
const MAX_SOURCES: usize = 128;
const MAX_MANIFEST_BYTES: u64 = 128 * 1024;
const MAX_NAME_BYTES: usize = 128;
const MAX_URL_BYTES: usize = 2048;
const MANIFEST_RELATIVE_PATH: &str = "modules/source/custom_sources.json";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceCategory {
    Wool,
    Weibo,
    Devtools,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CustomSource {
    pub category: SourceCategory,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Manifest {
    version: u32,
    sources: Vec<CustomSource>,
}

struct StagingFile {
    path: PathBuf,
    committed: bool,
}

impl Drop for StagingFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join(MANIFEST_RELATIVE_PATH)
}

pub(crate) fn validate_name(name: &str) -> Result<(), String> {
    if name.trim() != name
        || name.is_empty()
        || name.len() > MAX_NAME_BYTES
        || name.chars().any(char::is_control)
    {
        return Err(format!(
            "custom source name must be 1..={MAX_NAME_BYTES} bytes, have no surrounding whitespace, and contain no control characters"
        ));
    }
    Ok(())
}

/// Only public raw GitHub and jsDelivr module paths are accepted; credentials,
/// nonstandard ports, query strings, fragments, and traversal-like paths fail closed.
pub fn validate_url(url: &str) -> Result<(), String> {
    if url.len() > MAX_URL_BYTES
        || url.trim() != url
        || url.chars().any(char::is_control)
        || url.contains('\\')
    {
        return Err(format!(
            "custom source URL must be at most {MAX_URL_BYTES} bytes and contain no whitespace controls or backslashes"
        ));
    }
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "custom source URL must use lowercase https://".to_string())?;
    if rest.contains('?') || rest.contains('#') {
        return Err("custom source URL must not contain a query string or fragment".to_string());
    }
    let (authority, path) = rest
        .split_once('/')
        .ok_or_else(|| "custom source URL must include a module path".to_string())?;
    if authority.contains('@') {
        return Err("custom source URL must not contain credentials".to_string());
    }
    let host = authority.strip_suffix(":443").unwrap_or(authority);
    if host != "raw.githubusercontent.com" && host != "cdn.jsdelivr.net" {
        return Err(
            "custom source host must be raw.githubusercontent.com or cdn.jsdelivr.net".to_string(),
        );
    }
    if authority != host && authority != format!("{host}:443") {
        return Err("custom source URL must not use a nonstandard port".to_string());
    }
    if path.is_empty() {
        return Err("custom source URL must include a non-empty module path".to_string());
    }
    for segment in path.split('/') {
        let lower = segment.to_ascii_lowercase();
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || lower.contains("%2e")
            || lower.contains("%2f")
            || lower.contains("%5c")
        {
            return Err(
                "custom source URL contains an empty or traversal-like path segment".to_string(),
            );
        }
    }
    let lower_path = path.to_ascii_lowercase();
    if !lower_path.ends_with(".sgmodule") && !lower_path.ends_with(".module") {
        return Err("custom source URL must identify a .sgmodule or .module file".to_string());
    }
    Ok(())
}

fn validate_source(source: &CustomSource) -> Result<(), String> {
    validate_name(&source.name)?;
    validate_url(&source.url)
}

fn validate_sources(sources: &[CustomSource]) -> Result<(), String> {
    if sources.len() > MAX_SOURCES {
        return Err(format!(
            "custom source count exceeds the limit of {MAX_SOURCES}"
        ));
    }
    let mut names = HashSet::with_capacity(sources.len());
    for source in sources {
        validate_source(source)?;
        if !names.insert(source.name.to_lowercase()) {
            return Err(format!("duplicate custom source name: {:?}", source.name));
        }
    }
    Ok(())
}

fn load_manifest(path: &Path) -> Result<Option<Manifest>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!(
                "custom source manifest is not a regular file: {}",
                path.display()
            ));
        }
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            validate_missing_manifest_parent(path)?;
            return Ok(None);
        }
        Err(error) => {
            return Err(format!(
                "cannot inspect custom source manifest {}: {error}",
                path.display()
            ));
        }
    };
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(format!(
            "custom source manifest exceeds {MAX_MANIFEST_BYTES} bytes"
        ));
    }
    let mut file = File::open(path).map_err(|error| {
        format!(
            "cannot open custom source manifest {}: {error}",
            path.display()
        )
    })?;
    let opened = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened custom source manifest: {error}"))?;
    if !opened.is_file() || opened.len() > MAX_MANIFEST_BYTES {
        return Err(
            "custom source manifest changed to a non-regular or oversized file".to_string(),
        );
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            format!(
                "cannot read custom source manifest {}: {error}",
                path.display()
            )
        })?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(format!(
            "custom source manifest exceeds {MAX_MANIFEST_BYTES} bytes"
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("custom source manifest is not UTF-8: {error}"))?;
    let manifest: Manifest = serde_json::from_str(text)
        .map_err(|error| format!("invalid custom source manifest: {error}"))?;
    if manifest.version != MANIFEST_VERSION {
        return Err(format!(
            "unsupported custom source manifest version {}; supported version is {MANIFEST_VERSION}",
            manifest.version
        ));
    }
    validate_sources(&manifest.sources)?;
    Ok(Some(manifest))
}

fn validate_missing_manifest_parent(path: &Path) -> Result<(), String> {
    let mut ancestor = path.parent();
    while let Some(candidate) = ancestor {
        match fs::symlink_metadata(candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "custom source manifest parent is a symlink: {}",
                    candidate.display()
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "custom source manifest parent is not a directory: {}",
                    candidate.display()
                ));
            }
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = candidate.parent();
            }
            Err(error) => {
                return Err(format!(
                    "cannot inspect custom source manifest parent {}: {error}",
                    candidate.display()
                ));
            }
        }
    }
    Ok(())
}

/// Missing manifest means an empty source list; malformed or unsafe files are errors.
pub fn load(root: &Path) -> Result<Vec<CustomSource>, String> {
    Ok(load_manifest(&manifest_path(root))?
        .map(|manifest| manifest.sources)
        .unwrap_or_default())
}

fn check_destination(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(format!(
            "refusing to replace non-regular custom source manifest: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match fs::symlink_metadata(path) {
                Ok(_) => Err(format!(
                    "refusing to replace occupied custom source path: {}",
                    path.display()
                )),
                Err(metadata_error) if metadata_error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(())
                }
                Err(metadata_error) => Err(format!(
                    "cannot inspect custom source path {}: {metadata_error}",
                    path.display()
                )),
            }
        }
        Err(error) => Err(format!(
            "cannot inspect custom source path {}: {error}",
            path.display()
        )),
    }
}

fn create_temp_file(
    parent: &Path,
    destination_name: &std::ffi::OsStr,
) -> Result<(File, PathBuf), String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    for _ in 0..64 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut name = destination_name.to_os_string();
        name.push(format!(
            ".{}.{}.{}.tmp",
            std::process::id(),
            timestamp,
            sequence
        ));
        let path = parent.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot create staged custom source manifest: {error}"
                ));
            }
        }
    }
    Err("could not allocate a unique staged custom source manifest".to_string())
}

fn save_manifest(path: &Path, manifest: &Manifest) -> Result<(), String> {
    validate_sources(&manifest.sources)?;
    if manifest.version != MANIFEST_VERSION {
        return Err(format!(
            "unsupported custom source manifest version {}",
            manifest.version
        ));
    }
    check_destination(path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create custom source directory {}: {error}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("cannot encode custom source manifest: {error}"))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(format!(
            "encoded custom source manifest exceeds {MAX_MANIFEST_BYTES} bytes"
        ));
    }
    let (mut file, temp_path) = create_temp_file(parent, path.file_name().unwrap_or_default())?;
    let mut staging = StagingFile {
        path: temp_path,
        committed: false,
    };
    file.write_all(&bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("cannot write staged custom source manifest: {error}"))?;
    drop(file);
    check_destination(path)?;
    fs::rename(&staging.path, path).map_err(|error| {
        format!(
            "cannot atomically replace custom source manifest {}: {error}",
            path.display()
        )
    })?;
    staging.committed = true;
    Ok(())
}

/// Add one validated source and atomically persist the versioned manifest.
pub fn add(root: &Path, source: CustomSource) -> Result<(), String> {
    validate_source(&source)?;
    let path = manifest_path(root);
    let mut manifest = load_manifest(&path)?.unwrap_or(Manifest {
        version: MANIFEST_VERSION,
        sources: Vec::new(),
    });
    if manifest.sources.len() >= MAX_SOURCES {
        return Err(format!(
            "custom source count already reached the limit of {MAX_SOURCES}"
        ));
    }
    if manifest
        .sources
        .iter()
        .any(|existing| existing.name.eq_ignore_ascii_case(&source.name))
    {
        return Err(format!(
            "custom source name already exists: {:?}",
            source.name
        ));
    }
    manifest.sources.push(source);
    save_manifest(&path, &manifest)
}

/// Remove a source by its unique name and category; the absent manifest is the empty state.
pub fn remove(root: &Path, category: SourceCategory, name: &str) -> Result<(), String> {
    validate_name(name)?;
    let path = manifest_path(root);
    let mut manifest =
        load_manifest(&path)?.ok_or_else(|| "custom source manifest does not exist".to_string())?;
    let original_count = manifest.sources.len();
    manifest
        .sources
        .retain(|source| source.category != category || !source.name.eq_ignore_ascii_case(name));
    if manifest.sources.len() == original_count {
        return Err(format!("custom source not found: {:?}", name));
    }
    if manifest.sources.is_empty() {
        check_destination(&path)?;
        fs::remove_file(&path).map_err(|error| {
            format!(
                "cannot remove empty custom source manifest {}: {error}",
                path.display()
            )
        })
    } else {
        save_manifest(&path, &manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::{CustomSource, SourceCategory, add, load, remove, validate_url};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "script-hub-custom-sources-{}-{timestamp}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn source(name: &str) -> CustomSource {
            CustomSource {
                category: SourceCategory::Wool,
                name: name.to_string(),
                url: "https://raw.githubusercontent.com/example/repo/main/custom.sgmodule"
                    .to_string(),
            }
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_manifest_is_empty_and_add_remove_round_trips() {
        let root = TestRoot::new();
        assert!(load(&root.0).unwrap().is_empty());
        add(&root.0, TestRoot::source("One")).unwrap();
        assert_eq!(load(&root.0).unwrap(), vec![TestRoot::source("One")]);
        add(&root.0, TestRoot::source("Two")).unwrap();
        assert_eq!(
            load(&root.0).unwrap(),
            vec![TestRoot::source("One"), TestRoot::source("Two")]
        );
        remove(&root.0, SourceCategory::Wool, "One").unwrap();
        assert_eq!(load(&root.0).unwrap(), vec![TestRoot::source("Two")]);
        remove(&root.0, SourceCategory::Wool, "Two").unwrap();
        assert!(load(&root.0).unwrap().is_empty());
        assert!(!root.0.join("modules/source/custom_sources.json").exists());
    }

    #[test]
    fn missing_manifest_does_not_hide_a_non_directory_parent() {
        let root = TestRoot::new();
        fs::write(root.0.join("modules"), "occupied by file").unwrap();
        let error = load(&root.0).unwrap_err();
        // Unix may report ENOTDIR for the full path; Windows may report
        // NotFound and require the explicit ancestor check. Both must fail.
        assert!(error.contains("manifest"));
        assert_eq!(
            fs::read_to_string(root.0.join("modules")).unwrap(),
            "occupied by file"
        );
    }

    #[test]
    fn duplicate_names_and_unsafe_urls_are_rejected() {
        let root = TestRoot::new();
        add(&root.0, TestRoot::source("One")).unwrap();
        assert!(add(&root.0, TestRoot::source("one")).is_err());
        for url in [
            "http://raw.githubusercontent.com/example/repo/main/a.sgmodule",
            "https://user:secret@raw.githubusercontent.com/example/repo/a.sgmodule",
            "https://raw.githubusercontent.com.attacker.test/example/repo/a.sgmodule",
            "https://127.0.0.1/private.sgmodule",
            "https://raw.githubusercontent.com/example/repo/../private.sgmodule",
            "https://raw.githubusercontent.com/example/repo/%2e%2e/private.sgmodule",
            "https://raw.githubusercontent.com/example/repo/a.sgmodule?token=x",
            "https://raw.githubusercontent.com/example/repo/a.txt",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn corrupt_manifest_is_not_silently_replaced() {
        let root = TestRoot::new();
        let path = root.0.join("modules/source/custom_sources.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not json").unwrap();
        assert!(add(&root.0, TestRoot::source("One")).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "not json");
    }
}
