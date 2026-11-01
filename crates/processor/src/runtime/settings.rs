use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MAX_PREFERENCES_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Theme {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Accent {
    #[default]
    #[serde(rename = "blue")]
    Blue,
    #[serde(rename = "teal")]
    Teal,
    #[serde(rename = "amber")]
    Amber,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Language {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "zh_cn")]
    #[serde(alias = "ZH")]
    ZhCn,
    #[default]
    #[serde(rename = "en")]
    #[serde(alias = "EN")]
    En,
    #[serde(rename = "ja", alias = "JA")]
    Ja,
}

impl Language {
    /// Resolve the system choice from the native OS locale, then environment.
    /// Explicit language preferences are returned unchanged.
    pub fn resolved(self) -> Self {
        if self != Self::System {
            return self;
        }

        resolve_system_locale(system_locale(), environment_locale())
    }
}

fn language_for_locale(locale: &str) -> Language {
    let language = locale
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match language.as_str() {
        "zh" => Language::ZhCn,
        "ja" => Language::Ja,
        _ => Language::En,
    }
}

fn resolve_system_locale(native: Option<String>, environment: Option<String>) -> Language {
    native
        .or(environment)
        .as_deref()
        .map(language_for_locale)
        .unwrap_or(Language::En)
}

fn environment_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
}

fn system_locale() -> Option<String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        sys_locale::get_locale().filter(|value| !value.is_empty())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum WindowSize {
    #[serde(rename = "compact")]
    Compact,
    #[serde(rename = "standard")]
    #[default]
    Standard,
    #[serde(rename = "wide")]
    Wide,
}

impl WindowSize {
    pub fn dimensions(self) -> [f32; 2] {
        match self {
            Self::Compact => [900.0, 640.0],
            Self::Standard => [1160.0, 760.0],
            Self::Wide => [1440.0, 900.0],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Density {
    #[default]
    #[serde(rename = "comfortable")]
    Comfortable,
    #[serde(rename = "compact")]
    Compact,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Preferences {
    pub version: u32,
    pub root: String,
    pub singbox: String,
    pub theme: Theme,
    pub accent: Accent,
    pub language: Language,
    pub scale_percent: u16,
    pub window: WindowSize,
    pub density: Density,
    /// Opening protected folders is opt-in; merely launching the GUI stays local
    /// to its own settings until a workspace is explicitly opened.
    pub reopen_workspace: bool,
    pub record_history: bool,
    pub developer_mode: bool,
    pub diagnostic_logs: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            root: String::new(),
            singbox: String::new(),
            theme: Theme::default(),
            accent: Accent::default(),
            language: Language::default(),
            scale_percent: 100,
            window: WindowSize::default(),
            density: Density::default(),
            reopen_workspace: false,
            record_history: true,
            developer_mode: false,
            diagnostic_logs: false,
        }
    }
}

impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!(
                "unsupported preferences version {}; this application supports version 1",
                self.version
            ));
        }
        if !(80..=150).contains(&self.scale_percent) {
            return Err(format!(
                "scale_percent must be between 80 and 150, got {}",
                self.scale_percent
            ));
        }
        Ok(())
    }
}

/// Platform settings location. Unsupported platforms are reported rather than
/// silently selecting a nonstandard location.
#[cfg(target_os = "macos")]
pub fn preferences_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| "HOME is not set; cannot locate preferences".to_owned())?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("Script Hub")
        .join("settings.json"))
}

#[cfg(target_os = "windows")]
pub fn preferences_path() -> Result<PathBuf, String> {
    let app_data = std::env::var_os("APPDATA")
        .ok_or_else(|| "APPDATA is not set; cannot locate preferences".to_owned())?;
    Ok(PathBuf::from(app_data)
        .join("ScriptHub")
        .join("settings.json"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn preferences_path() -> Result<PathBuf, String> {
    Err("preferences storage is supported only on macOS and Windows".to_owned())
}

/// Load the settings file. `Ok(None)` means the path is truly absent; all other
/// filesystem, decoding, schema, and validation errors remain visible.
pub fn load(path: &Path) -> Result<Option<Preferences>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!(
                "preferences path is not a regular file: {}",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::verify_missing_directory(path)?;
            return Ok(None);
        }
        Err(error) => {
            return Err(format!(
                "cannot inspect preferences path {}: {error}",
                path.display()
            ));
        }
    }

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            return Err(format!(
                "cannot open preferences {}: {error}",
                path.display()
            ));
        }
    };

    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect preferences {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "preferences path is not a regular file: {}",
            path.display()
        ));
    }
    if metadata.len() > MAX_PREFERENCES_BYTES {
        return Err(format!(
            "preferences file exceeds the {} KiB size limit: {}",
            MAX_PREFERENCES_BYTES / 1024,
            path.display()
        ));
    }

    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_PREFERENCES_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read preferences {}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_PREFERENCES_BYTES {
        return Err(format!(
            "preferences file exceeds the {} KiB size limit: {}",
            MAX_PREFERENCES_BYTES / 1024,
            path.display()
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("preferences {} are not UTF-8: {error}", path.display()))?;
    let preferences: Preferences = serde_json::from_str(text)
        .map_err(|error| format!("invalid preferences {}: {error}", path.display()))?;
    preferences.validate()?;
    Ok(Some(preferences))
}

/// Persist preferences using a unique, exclusive staging file beside the
/// destination. Existing malformed or unsupported settings are never replaced.
pub fn save(path: &Path, preferences: &Preferences) -> Result<(), String> {
    preferences.validate()?;
    let bytes = serde_json::to_vec_pretty(preferences)
        .map_err(|error| format!("cannot encode preferences: {error}"))?;
    if bytes.len() as u64 > MAX_PREFERENCES_BYTES {
        return Err(format!(
            "encoded preferences exceed {MAX_PREFERENCES_BYTES} bytes"
        ));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create preferences directory {}: {error}",
            parent.display()
        )
    })?;

    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!(
                "refusing to replace non-regular preferences path: {}",
                path.display()
            ));
        }
        Ok(_) => {
            // Do not overwrite a damaged or newer-version file without an
            // explicit recovery/backup choice from the caller.
            load(path)?.ok_or_else(|| {
                format!("preferences disappeared while saving: {}", path.display())
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A dangling symlink is still an occupied path and must not be replaced.
            match fs::symlink_metadata(path) {
                Ok(_) => {
                    return Err(format!(
                        "refusing to replace occupied preferences path: {}",
                        path.display()
                    ));
                }
                Err(metadata_error) if metadata_error.kind() == std::io::ErrorKind::NotFound => {}
                Err(metadata_error) => {
                    return Err(format!(
                        "cannot inspect preferences path {}: {metadata_error}",
                        path.display()
                    ));
                }
            }
        }
        Err(error) => {
            return Err(format!(
                "cannot inspect preferences path {}: {error}",
                path.display()
            ));
        }
    }

    let (mut file, temp_path) = create_temp_file(parent, path.file_name().unwrap_or_default())?;
    let mut staging = StagingFile::new(temp_path);
    file.write_all(&bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("cannot write staged preferences: {error}"))?;
    drop(file);

    // On platforms where rename cannot replace an existing destination this
    // safely fails while retaining the old file; no delete-before-rename gap.
    fs::rename(&staging.path, path).map_err(|error| {
        format!(
            "cannot atomically replace preferences {}: {error}",
            path.display()
        )
    })?;
    staging.committed = true;
    Ok(())
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
                    "cannot create staged preferences beside {}: {error}",
                    parent.display()
                ));
            }
        }
    }
    Err(format!(
        "could not allocate a unique staged file beside {}",
        parent.display()
    ))
}

struct StagingFile {
    path: PathBuf,
    committed: bool,
}

impl StagingFile {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }
}

impl Drop for StagingFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "script-hub-settings-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_file_and_partial_document_use_defaults() {
        let dir = TestDir::new();
        let path = dir.path().join("settings.json");
        assert_eq!(load(&path).unwrap(), None);
        fs::write(&path, r#"{"root":"/rules"}"#).unwrap();
        let loaded = load(&path).unwrap().unwrap();
        assert_eq!(loaded.root, "/rules");
        assert_eq!(loaded, Preferences::default().with_root("/rules"));
    }

    #[test]
    fn diagnostic_storage_requires_explicit_opt_in() {
        let preferences: Preferences = serde_json::from_str("{}").unwrap();
        assert!(preferences.record_history);
        assert!(!preferences.developer_mode);
        assert!(!preferences.diagnostic_logs);
    }

    #[test]
    fn new_preferences_use_english() {
        assert_eq!(Preferences::default().language, Language::En);
    }

    #[test]
    fn preferences_without_a_language_use_english() {
        let preferences: Preferences = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(preferences.language, Language::En);
    }

    #[test]
    fn saved_language_choices_are_preserved() {
        let dir = TestDir::new();
        let path = dir.path().join("settings.json");
        for language in [Language::ZhCn, Language::En, Language::System, Language::Ja] {
            let preferences = Preferences {
                language,
                ..Preferences::default()
            };
            save(&path, &preferences).unwrap();
            assert_eq!(load(&path).unwrap().unwrap().language, language);
        }
    }

    #[test]
    fn round_trip_persists_all_preferences() {
        let dir = TestDir::new();
        let path = dir.path().join("nested/settings.json");
        let preferences = Preferences {
            root: "/rules".to_owned(),
            singbox: "/tools/sing-box".to_owned(),
            theme: Theme::Dark,
            accent: Accent::Teal,
            language: Language::ZhCn,
            scale_percent: 125,
            window: WindowSize::Wide,
            density: Density::Compact,
            ..Preferences::default()
        };

        save(&path, &preferences).unwrap();
        assert_eq!(load(&path).unwrap(), Some(preferences));
        let updated = Preferences {
            theme: Theme::Light,
            language: Language::Ja,
            ..Preferences::default()
        };
        save(&path, &updated).unwrap();
        assert_eq!(load(&path).unwrap(), Some(updated));
        assert_eq!(WindowSize::Wide.dimensions(), [1440.0, 900.0]);
    }

    #[test]
    fn oversized_save_cannot_create_an_unreadable_preferences_file() {
        let dir = TestDir::new();
        let path = dir.path().join("settings.json");
        save(&path, &Preferences::default()).unwrap();
        let original = fs::read(&path).unwrap();
        let oversized = Preferences {
            root: "x".repeat(MAX_PREFERENCES_BYTES as usize),
            ..Preferences::default()
        };
        assert!(save(&path, &oversized).unwrap_err().contains("exceed"));
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(load(&path).unwrap(), Some(Preferences::default()));
    }

    #[test]
    fn system_language_parser_handles_os_and_posix_locale_forms() {
        assert_eq!(language_for_locale("zh-Hans-CN"), Language::ZhCn);
        assert_eq!(language_for_locale("zh_CN.UTF-8"), Language::ZhCn);
        assert_eq!(language_for_locale("ja-JP"), Language::Ja);
        assert_eq!(language_for_locale("JA_JP.UTF-8"), Language::Ja);
        assert_eq!(language_for_locale("en_US.UTF-8"), Language::En);
        assert_eq!(language_for_locale("C"), Language::En);
    }

    #[test]
    fn system_locale_wins_and_environment_is_the_fallback() {
        assert_eq!(
            resolve_system_locale(Some("ja-JP".to_string()), Some("zh_CN.UTF-8".to_string())),
            Language::Ja
        );
        assert_eq!(
            resolve_system_locale(None, Some("zh_CN.UTF-8".to_string())),
            Language::ZhCn
        );
        assert_eq!(resolve_system_locale(None, None), Language::En);
        assert_eq!(Language::Ja.resolved(), Language::Ja);
    }

    #[test]
    fn invalid_json_is_reported_and_preserved_by_save() {
        let dir = TestDir::new();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{broken").unwrap();
        let before = fs::read(&path).unwrap();
        assert!(load(&path).unwrap_err().contains("invalid preferences"));
        assert!(save(&path, &Preferences::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn oversized_preferences_are_rejected_and_preserved_by_save() {
        let dir = TestDir::new();
        let path = dir.path().join("settings.json");
        let before = vec![b' '; MAX_PREFERENCES_BYTES as usize + 1];
        fs::write(&path, &before).unwrap();

        assert!(load(&path).unwrap_err().contains("size limit"));
        assert!(
            save(&path, &Preferences::default())
                .unwrap_err()
                .contains("size limit")
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn unsupported_version_and_invalid_scale_are_rejected() {
        let dir = TestDir::new();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"version":2}"#).unwrap();
        assert!(
            load(&path)
                .unwrap_err()
                .contains("unsupported preferences version")
        );
        fs::write(&path, r#"{"version":1,"scale_percent":79}"#).unwrap();
        assert!(load(&path).unwrap_err().contains("scale_percent"));
        let invalid = Preferences {
            scale_percent: 151,
            ..Preferences::default()
        };
        assert!(save(&path, &invalid).unwrap_err().contains("scale_percent"));
    }

    #[test]
    fn occupied_directory_is_never_replaced() {
        let dir = TestDir::new();
        let target = dir.path().join("settings.json");
        fs::create_dir(&target).unwrap();
        assert!(save(&target, &Preferences::default()).is_err());
        assert!(target.is_dir());
    }

    #[test]
    fn missing_settings_under_file_ancestor_is_not_treated_as_absent() {
        let dir = TestDir::new();
        let blocker = dir.path().join("not-a-directory");
        fs::write(&blocker, "file").unwrap();
        let path = blocker.join("settings.json");
        assert!(load(&path).is_err());
    }

    #[test]
    fn failed_save_leaves_no_staging_file() {
        let dir = TestDir::new();
        let target = dir.path().join("settings.json");
        fs::write(&target, r#"{"version":2}"#).unwrap();
        let before = fs::read(&target).unwrap();
        assert!(save(&target, &Preferences::default()).is_err());
        assert_eq!(fs::read(&target).unwrap(), before);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    impl Preferences {
        fn with_root(mut self, root: &str) -> Self {
            self.root = root.to_owned();
            self
        }
    }
}
