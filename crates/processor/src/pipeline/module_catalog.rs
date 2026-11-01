use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const CDN: &str = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/";
const RAW: &str = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/";
const CATEGORIES: [(&str, &str); 3] = [
    ("amplify_nexus", "🛠️ Amplify Nexus › 增幅枢纽"),
    ("head_expanse", "🔝 Head Expanse › 首端扩域"),
    ("narrow_pierce", "🎯 Narrow Pierce › 窄域穿刺"),
];

#[derive(Clone, Serialize)]
struct ModuleInfo {
    id: String,
    filename: String,
    category: String,
    path: String,
    has_arguments: bool,
    merged_into: String,
    install_url: String,
    github_install_url: String,
    name: String,
    desc: String,
    tags: Vec<String>,
    author: String,
    date: String,
    icon: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    essential: Option<bool>,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|part| utf8_percent_encode(part, NON_ALPHANUMERIC).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn scan_modules(root: &Path, platform: &str, extension: &str) -> Result<Vec<ModuleInfo>, String> {
    let mut modules = Vec::new();
    for (category, _) in CATEGORIES {
        let dir = root.join("modules").join(platform).join(category);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                crate::verify_missing_directory(&dir)?;
                continue;
            }
            Err(error) => return Err(format!("cannot scan {}: {error}", dir.display())),
        };
        let mut paths: Vec<PathBuf> = entries
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("cannot scan {}: {error}", dir.display()))?
            .into_iter()
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == extension))
            .collect();
        paths.sort();
        for path in paths {
            let content = fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let meta = crate::merge_bundles::module_metadata_pub(&content);
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let rel = relative_path(root, &path);
            let name = meta.get("name").cloned().unwrap_or_else(|| {
                filename
                    .trim_end_matches(extension)
                    .trim_end_matches('.')
                    .to_string()
            });
            let tags: Vec<String> = meta
                .get("tag")
                .map(|tags| {
                    tags.split(',')
                        .map(str::trim)
                        .filter(|tag| !tag.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let deprecated = name.contains("已退役")
                || tags
                    .iter()
                    .any(|tag| tag.eq_ignore_ascii_case("deprecated"));
            let mirror = path.parent().unwrap_or(&dir).join("github").join(&filename);
            let mirror_rel = relative_path(root, &mirror);
            modules.push(ModuleInfo {
                id: filename
                    .trim_end_matches(extension)
                    .trim_end_matches('.')
                    .to_string(),
                filename,
                category: category.to_string(),
                path: rel.clone(),
                has_arguments: meta.contains_key("arguments"),
                merged_into: if deprecated {
                    "PROMAX".to_string()
                } else {
                    String::new()
                },
                install_url: format!("{CDN}{}", encode_path(&rel)),
                github_install_url: if mirror.is_file() {
                    format!("{RAW}{}", encode_path(&mirror_rel))
                } else {
                    format!("{RAW}{}", encode_path(&rel))
                },
                name,
                desc: meta.get("desc").cloned().unwrap_or_default(),
                tags,
                author: meta.get("author").cloned().unwrap_or_default(),
                date: meta.get("date").cloned().unwrap_or_default(),
                icon: meta.get("icon").cloned().unwrap_or_default(),
                essential: deprecated.then_some(false),
                note: if deprecated {
                    "兼容旧链接的无操作占位，请勿重复安装".to_string()
                } else {
                    String::new()
                },
            });
        }
    }
    Ok(modules)
}

/// Returns the platform module catalog without writing generated files.
pub fn catalog_snapshot(root: &Path, shadowrocket: bool) -> Result<Value, String> {
    let (platform, extension) = if shadowrocket {
        ("shadowrocket", "module")
    } else {
        ("surge", "sgmodule")
    };
    serde_json::to_value(scan_modules(root, platform, extension)?)
        .map_err(|error| format!("cannot encode module catalog: {error}"))
}

#[cfg(test)]
mod tests {
    use super::catalog_snapshot;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn catalog_snapshot_is_read_only_and_includes_retired_modules() {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("module-snapshot-{id}"));
        let surge = root.join("modules/surge/head_expanse");
        fs::create_dir_all(&surge).unwrap();
        fs::write(surge.join("Old.sgmodule"), "#!name=Old（已退役）\n").unwrap();

        let snapshot = catalog_snapshot(&root, false).unwrap();
        assert_eq!(snapshot[0]["merged_into"], "PROMAX");
        assert_eq!(snapshot[0]["essential"], false);
        assert_eq!(
            snapshot[0]["github_install_url"],
            "https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/surge/head%5Fexpanse/Old%2Esgmodule"
        );
        assert!(!root.join("modules/helper").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unreadable_category_is_an_error_not_a_silent_empty_catalog() {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("module-scan-error-{id}"));
        let surge = root.join("modules/surge");
        fs::create_dir_all(&surge).unwrap();
        fs::write(surge.join("head_expanse"), "not a directory").unwrap();
        assert!(catalog_snapshot(&root, false).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
