use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
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

#[derive(Serialize)]
struct HelperGroup {
    name: String,
    items: Vec<Value>,
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
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .flatten()
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
                    format!("{CDN}{}", encode_path(&rel))
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

fn generated_version(modules: &[ModuleInfo]) -> String {
    modules
        .iter()
        .filter_map(|module| (!module.date.is_empty()).then_some(module.date.as_str()))
        .max()
        .unwrap_or("deterministic")
        .to_string()
}

fn helper_groups(modules: &[ModuleInfo], shadowrocket: bool) -> BTreeMap<String, HelperGroup> {
    let mut groups = BTreeMap::new();
    for (category, name) in CATEGORIES {
        groups.insert(
            category.to_string(),
            HelperGroup {
                name: name.to_string(),
                items: Vec::new(),
            },
        );
    }
    for module in modules
        .iter()
        .filter(|module| module.merged_into.is_empty())
    {
        let badge = if module.name.contains("PROMAX") {
            "🔥"
        } else if module.name.contains("Script Hub") {
            "⭐"
        } else {
            ""
        };
        let url = &module.install_url;
        let id = format!("{:x}", md5::compute(url.as_bytes()));
        let desc = if shadowrocket && !module.desc.starts_with("[🚀SR]") {
            format!("[🚀SR] {}", module.desc)
        } else {
            module.desc.clone()
        };
        if let Some(group) = groups.get_mut(&module.category) {
            group.items.push(json!({
                "id": id,
                "name": module.name,
                "desc": desc,
                "author": module.author,
                "date": module.date,
                "icon": module.icon,
                "url": url,
                "github_url": module.github_install_url,
                "badge": badge,
                "filename": module.filename,
            }));
        }
    }
    groups
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot encode {}: {error}", path.display()))?
        + "\n";
    if crate::safe_write_file_internal(path, &content, true) {
        Ok(())
    } else {
        Err(format!("cannot write {}", path.display()))
    }
}

fn update_helper_html(path: &Path, surge: &Value, shadowrocket: &Value) -> Result<(), String> {
    let html = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let inline_json = |value: &Value| {
        serde_json::to_string(value)
            .map(|json| {
                json.replace('&', "\\u0026")
                    .replace('<', "\\u003c")
                    .replace('>', "\\u003e")
            })
            .map_err(|error| error.to_string())
    };
    let surge = inline_json(surge)?;
    let shadowrocket = inline_json(shadowrocket)?;
    let mut replaced = 0;
    let mut output = Vec::new();
    for line in html.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        if trimmed.starts_with("const surgeData = ") {
            output.push(format!("{indent}const surgeData = {surge};"));
            replaced += 1;
        } else if trimmed.starts_with("const srData = ") {
            output.push(format!("{indent}const srData = {shadowrocket};"));
            replaced += 1;
        } else {
            output.push(line.to_string());
        }
    }
    if replaced != 2 {
        return Err("module helper template is missing embedded data slots".to_string());
    }
    let content = output.join("\n") + "\n";
    if crate::safe_write_file_internal(path, &content, true) {
        Ok(())
    } else {
        Err(format!("cannot write {}", path.display()))
    }
}

pub fn refresh(root: &Path) -> Result<(), String> {
    let surge = scan_modules(root, "surge", "sgmodule")?;
    let shadowrocket = scan_modules(root, "shadowrocket", "module")?;
    let helper = root.join("modules/helper");
    fs::create_dir_all(&helper).map_err(|error| error.to_string())?;
    let generated = generated_version(&surge);

    write_json(
        &helper.join("modules_data.json"),
        &json!({
            "generated": generated,
            "total": surge.len(),
            "policy": {
                "adblock": "仅安装 PROMAX；旧 AdBlock Helper 为无操作兼容占位",
                "features": "解锁、增强、翻译模块保持独立安装"
            },
            "modules": surge,
        }),
    )?;
    let sr_groups = helper_groups(&shadowrocket, true);
    write_json(
        &helper.join("shadowrocket_modules_data.json"),
        &json!({
            "generated": generated,
            "total": shadowrocket.iter().filter(|module| module.merged_into.is_empty()).count(),
            "categories": sr_groups,
        }),
    )?;

    let sr_keys: HashSet<(String, String)> = shadowrocket
        .iter()
        .map(|module| (module.category.clone(), module.id.clone()))
        .collect();
    let (compatible, surge_only): (Vec<_>, Vec<_>) = surge
        .iter()
        .partition(|module| sr_keys.contains(&(module.category.clone(), module.id.clone())));
    write_json(
        &helper.join("modules_compatibility.json"),
        &json!({
            "generated": generated,
            "total": surge.len(),
            "compatible": compatible.len(),
            "surge_only": surge_only.len(),
            "modules": {
                "compatible": compatible.iter().map(|module| json!({"name": module.name, "category": module.category})).collect::<Vec<_>>(),
                "surge_only": surge_only.iter().map(|module| json!({"name": module.name, "category": module.category, "issues": ["无对应 Shadowrocket 发布产物"]})).collect::<Vec<_>>()
            }
        }),
    )?;
    write_json(
        &helper.join("shadowrocket_conversion_log.json"),
        &json!({
            "generated": generated,
            "stats": {"total": surge.len(), "converted": compatible.len(), "failed": surge_only.len()},
            "log": surge.iter().map(|module| {
                let ok = sr_keys.contains(&(module.category.clone(), module.id.clone()));
                json!({
                    "src": module.filename,
                    "dst": format!("{}.module", module.id),
                    "status": if ok { "ok" } else { "missing" }
                })
            }).collect::<Vec<_>>()
        }),
    )?;

    let surge_groups =
        serde_json::to_value(helper_groups(&surge, false)).map_err(|error| error.to_string())?;
    let sr_groups = serde_json::to_value(sr_groups).map_err(|error| error.to_string())?;
    update_helper_html(
        &helper.join("surge_module_helper.html"),
        &surge_groups,
        &sr_groups,
    )?;
    println!(
        "[INFO] refreshed module catalog: Surge={}, Shadowrocket={}",
        surge.len(),
        shadowrocket.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::refresh;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn refreshes_catalog_and_hides_deprecated_helper_from_install_ui() {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("module-catalog-{id}"));
        let surge = root.join("modules/surge/head_expanse");
        let sr = root.join("modules/shadowrocket/head_expanse");
        let helper = root.join("modules/helper");
        fs::create_dir_all(&surge).unwrap();
        fs::create_dir_all(&sr).unwrap();
        fs::create_dir_all(&helper).unwrap();
        fs::write(
            surge.join("PROMAX.sgmodule"),
            "#!name=PROMAX\n#!date=2026-08-23\n",
        )
        .unwrap();
        fs::write(
            sr.join("PROMAX.module"),
            "#!name=PROMAX\n#!date=2026-08-23\n",
        )
        .unwrap();
        fs::write(
            surge.join("Helper.sgmodule"),
            "#!name=Helper（已退役）\n#!tag=Deprecated\n",
        )
        .unwrap();
        fs::write(
            helper.join("surge_module_helper.html"),
            "<script>\nconst surgeData = {};\nconst srData = {};\n</script>\n",
        )
        .unwrap();

        refresh(&root).unwrap();

        let html = fs::read_to_string(helper.join("surge_module_helper.html")).unwrap();
        assert!(html.contains("PROMAX"));
        assert!(!html.contains("Helper（已退役）"));
        let catalog = fs::read_to_string(helper.join("modules_data.json")).unwrap();
        assert!(catalog.contains("兼容旧链接的无操作占位"));
        fs::remove_dir_all(root).unwrap();
    }
}
