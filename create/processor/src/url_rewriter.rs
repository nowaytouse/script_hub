use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

static MOCK_REPLACEMENTS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/reject-200\.txt"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-200.txt"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/blank\.txt"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank.txt"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/reject-dict\.json"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-dict.json"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/reject-img\.gif"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-img.gif"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/blank_dict\.json\.js"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank_dict.json.js"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/blank\.gif"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank.gif"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/blank_dict\.json"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank_dict.json"),
        (Regex::new(r#"(?i)https://ghproxy\.net/https://github\.com/([^/]+)/([^/]+)/releases/download/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)"#).unwrap(), "https://github.com/${1}/${2}/releases/download/${3}/${4}"),
        (Regex::new(r#"(?i)https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)"#).unwrap(), "https://cdn.jsdelivr.net/gh/${1}/${2}@${3}/${4}"),
        (Regex::new(r#"(?i)(^|[^/])https://github\.com/([^/]+)/([^/]+)/raw/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)"#).unwrap(), "${1}https://cdn.jsdelivr.net/gh/${2}/${3}@${4}/${5}"),
    ]
});

static REVERSE_CDN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(^|[^/])https://cdn\.jsdelivr\.net/gh/([^/]+)/([^/@]+)@([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)"#).unwrap()
});
static GENERATED_DATE_SUFFIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s*-\s*\[\d{4}-\d{2}-\d{2}(?:[ T]\d{2}:\d{2}:\d{2})?\]").unwrap());
static MODULE_NAME_KEY: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^#!name\s*=").unwrap());
static GENERATED_README_DATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(自动生成于\s*`|generated\s+(?:at|on)\s+`)[^`]+`").unwrap());
static INLINE_CATALOG_DATE_FIELD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)(\"(?:generated|date)\"\s*:\s*)\"[^\"]*\""#).unwrap());

fn is_volatile_metadata(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase().replace(' ', "");
    [
        "#!date=",
        "#!version=20",
        "#!updated=",
        "#!last-updated=",
        "#!generated-at=",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn strip_trailing_comment(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut quoted = None;
    let mut escaped = false;
    for index in 0..bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' && quoted.is_some() {
            escaped = true;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            if quoted == Some(byte) {
                quoted = None;
            } else if quoted.is_none() {
                quoted = Some(byte);
            }
            continue;
        }
        if quoted.is_none()
            && (byte == b'#' || (byte == b'/' && bytes.get(index + 1) == Some(&b'/')))
        {
            let preceded_by_space = index == 0 || bytes[index - 1].is_ascii_whitespace();
            if preceded_by_space {
                return line[..index].trim_end().to_string();
            }
        }
    }
    line.trim().to_string()
}

fn normalize_rule_spacing(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut quoted = None;
    let mut pending_space = false;
    for character in line.chars() {
        if character == '"' || character == '\'' {
            if quoted == Some(character) {
                quoted = None;
            } else if quoted.is_none() {
                quoted = Some(character);
            }
            if pending_space {
                output.push(' ');
                pending_space = false;
            }
            output.push(character);
        } else if quoted.is_none() && character.is_whitespace() {
            pending_space = !output.ends_with(',');
        } else if quoted.is_none() && character == ',' {
            while output.ends_with(' ') {
                output.pop();
            }
            output.push(',');
            pending_space = false;
        } else {
            if pending_space {
                output.push(' ');
                pending_space = false;
            }
            output.push(character);
        }
    }
    output.trim().to_string()
}

fn normalize_assignment_key(line: &str) -> String {
    line.split_once('=').map_or_else(
        || line.trim().to_string(),
        |(key, value)| format!("{}={}", key.trim(), value.trim()),
    )
}

fn flush_force_http_engine_hosts(output: &mut Vec<String>, hosts: &mut Vec<String>) {
    if !hosts.is_empty() {
        output.push(format!(
            "force-http-engine-hosts = %APPEND% {}",
            hosts.join(", ")
        ));
        hosts.clear();
    }
}

pub(crate) fn normalize_force_http_engine_hosts(content: &str) -> String {
    let mut output = Vec::new();
    let mut in_general = false;
    let mut hosts = Vec::new();
    let mut seen = HashSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_general {
                flush_force_http_engine_hosts(&mut output, &mut hosts);
            }
            in_general = trimmed.eq_ignore_ascii_case("[General]");
            output.push(line.to_string());
            continue;
        }
        if in_general
            && let Some((key, value)) = trimmed.split_once('=')
            && key.trim().eq_ignore_ascii_case("force-http-engine-hosts")
            && let Some(value) = value.trim().strip_prefix("%APPEND%")
        {
            for host in value
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
            {
                if !host.starts_with("*:") && seen.insert(host.to_ascii_lowercase()) {
                    hosts.push(host.to_string());
                }
            }
            continue;
        }
        output.push(line.to_string());
    }
    if in_general {
        flush_force_http_engine_hosts(&mut output, &mut hosts);
    }
    let mut normalized = output.join("\n");
    if content.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

fn canonical_json(value: serde_json::Value, module_catalog: bool) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let mut sorted = BTreeMap::new();
            for (key, value) in object {
                let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
                if [
                    "updated",
                    "updatedat",
                    "generatedat",
                    "lastupdated",
                    "timestamp",
                ]
                .contains(&normalized.as_str())
                    || (module_catalog && matches!(normalized.as_str(), "generated" | "date"))
                {
                    continue;
                }
                sorted.insert(key, canonical_json(value, module_catalog));
            }
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values
                .into_iter()
                .map(|value| canonical_json(value, module_catalog))
                .collect(),
        ),
        serde_json::Value::String(value) if module_catalog => {
            serde_json::Value::String(GENERATED_DATE_SUFFIX.replace(&value, "").into_owned())
        }
        value => value,
    }
}

pub fn semantic_content_for_path_pub(path: &Path, text: &str) -> String {
    if path
        .file_name()
        .is_some_and(|name| name == "surge_module_helper.html")
    {
        let without_fields = INLINE_CATALOG_DATE_FIELD.replace_all(text, "${1}\"<volatile>\"");
        return GENERATED_DATE_SUFFIX
            .replace_all(&without_fields, "")
            .into_owned();
    }
    if path
        .extension()
        .is_some_and(|extension| extension == "json")
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(text)
    {
        let module_catalog = path
            .to_string_lossy()
            .replace('\\', "/")
            .contains("modules/helper/");
        if let Ok(canonical) = serde_json::to_string(&canonical_json(value, module_catalog)) {
            return canonical;
        }
    }

    let is_module = path.extension().is_some_and(|extension| {
        matches!(extension.to_str(), Some("sgmodule" | "module" | "plugin"))
    });
    let mut current_section = String::new();
    let mut lines = Vec::new();
    for line in text.lines() {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with("//") || is_volatile_metadata(stripped) {
            continue;
        }
        if is_module {
            if is_volatile_metadata(stripped) {
                continue;
            }
            if stripped.starts_with("#!") || (stripped.starts_with('[') && stripped.ends_with(']'))
            {
                if stripped.starts_with('[') {
                    current_section = stripped[1..stripped.len() - 1].to_ascii_lowercase();
                }
                let metadata = if MODULE_NAME_KEY.is_match(stripped) {
                    let normalized = GENERATED_DATE_SUFFIX.replace(stripped, "");
                    normalized.split_once('=').map_or_else(
                        || normalized.to_string(),
                        |(key, value)| format!("{}={}", key.trim(), value.trim()),
                    )
                } else {
                    stripped.split_once('=').map_or_else(
                        || stripped.to_string(),
                        |(key, value)| format!("{}={}", key.trim(), value.trim()),
                    )
                };
                lines.push(metadata);
                continue;
            }
            if stripped.starts_with('#') {
                continue;
            }
        } else if stripped.starts_with('#') {
            continue;
        }
        let canonical = if path.extension().is_some_and(|extension| extension == "md") {
            GENERATED_README_DATE
                .replace(stripped, "generated-at `<volatile>`")
                .to_string()
        } else if is_module && current_section == "rule" {
            normalize_rule_spacing(&strip_trailing_comment(stripped))
        } else if is_module && matches!(current_section.as_str(), "general" | "host" | "mitm") {
            normalize_assignment_key(&normalize_rule_spacing(&strip_trailing_comment(stripped)))
        } else if is_module && current_section == "script" {
            normalize_assignment_key(&strip_trailing_comment(stripped))
        } else {
            strip_trailing_comment(stripped)
        };
        if !canonical.is_empty() {
            lines.push(canonical);
        }
    }

    if path
        .extension()
        .is_some_and(|extension| extension == "list")
    {
        let mut unique = BTreeSet::new();
        unique.extend(lines);
        return unique.into_iter().collect::<Vec<_>>().join("\n");
    }
    lines.join("\n")
}

pub fn get_semantic_content_pub(text: &str) -> String {
    semantic_content_for_path_pub(Path::new("ruleset.list"), text)
}

pub fn is_pipeline_source_path_pub(path: &Path) -> bool {
    let path = path.to_string_lossy().replace('\\', "/");
    path.contains("/rulesets/Sources/") || path.contains("/modules/source/")
}

fn safe_write_file(path: &Path, content: &str) -> std::io::Result<()> {
    if path.exists()
        && let Ok(old_content) = fs::read_to_string(path)
    {
        let old_stripped = semantic_content_for_path_pub(path, &old_content);
        let new_stripped = semantic_content_for_path_pub(path, content);
        if old_stripped == new_stripped {
            return Ok(());
        }
    }
    if crate::safe_write_file_internal(path, content, true) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "atomic URL rewrite failed for {}",
            path.display()
        )))
    }
}

fn is_github_source(path: &Path) -> bool {
    for comp in path.components() {
        if let Some(s) = comp.as_os_str().to_str()
            && s == "github"
        {
            return true;
        }
    }
    false
}

pub fn copy_github_variants(root_dir: &str) {
    let root = Path::new(root_dir);
    let modules_dir = root.join("modules");
    let rulesets_dir = root.join("rulesets");

    let dirs_to_copy = vec![
        modules_dir.join("surge").join("amplify_nexus"),
        modules_dir.join("surge").join("head_expanse"),
        modules_dir.join("shadowrocket").join("amplify_nexus"),
        modules_dir.join("shadowrocket").join("head_expanse"),
        rulesets_dir.join("RULE-SET"),
        rulesets_dir.join("AdBlock"),
    ];

    let skip_copy: HashSet<&str> = [
        "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
        "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module",
    ]
    .iter()
    .cloned()
    .collect();

    for dir in dirs_to_copy {
        if !dir.exists() {
            continue;
        }
        let github_dir = dir.join("github");
        let _ = fs::create_dir_all(&github_dir);
        let mut expected = HashSet::new();

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                let file_name_os = entry.file_name();
                let file_name = file_name_os.to_string_lossy();

                if file_type.is_dir() || file_name == "github" {
                    continue;
                }

                if file_name.ends_with(".sgmodule")
                    || file_name.ends_with(".module")
                    || file_name.ends_with(".list")
                {
                    expected.insert(file_name.into_owned());
                    if skip_copy.contains(file_name_os.to_string_lossy().as_ref()) {
                        continue;
                    }
                    let src_path = entry.path();
                    let dest_path = github_dir.join(file_name_os);

                    if let Ok(content) = fs::read_to_string(&src_path) {
                        let new_content = REVERSE_CDN_REGEX.replace_all(
                            &content,
                            "${1}https://raw.githubusercontent.com/${2}/${3}/${4}/${5}",
                        );
                        let _ = safe_write_file(&dest_path, &new_content);
                    }
                }
            }
        }

        if let Ok(entries) = fs::read_dir(&github_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.path().is_file()
                    && (name.ends_with(".sgmodule")
                        || name.ends_with(".module")
                        || name.ends_with(".list"))
                    && !expected.contains(&name)
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

pub fn run_url_rewrites(directory: &str) -> i32 {
    let path = Path::new(directory);
    if !path.exists() {
        return 0;
    }

    let valid_exts: HashSet<&str> = ["sgmodule", "module", "list", "conf", "json", "html", "md"]
        .iter()
        .cloned()
        .collect();
    let mut modified_count = 0;

    for entry in WalkDir::new(directory).into_iter().filter_map(|e| e.ok()) {
        let entry_path = entry.path();

        if entry.file_type().is_dir() {
            continue;
        }

        if entry_path.to_string_lossy().contains(".git") {
            continue;
        }
        if is_github_source(entry_path) || is_pipeline_source_path_pub(entry_path) {
            continue;
        }

        if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
            if !valid_exts.contains(ext) {
                continue;
            }
        } else {
            continue;
        }

        if let Ok(content) = fs::read_to_string(entry_path) {
            let orig_content = content.clone();
            let mut new_content = if entry_path
                .extension()
                .is_some_and(|extension| matches!(extension.to_str(), Some("sgmodule" | "module")))
            {
                normalize_force_http_engine_hosts(&content)
            } else {
                content
            };

            for (re, repl) in MOCK_REPLACEMENTS.iter() {
                new_content = re.replace_all(&new_content, *repl).to_string();
            }

            if new_content != orig_content {
                let _ = safe_write_file(entry_path, &new_content);
                modified_count += 1;
            }
        }
    }
    modified_count
}

#[cfg(test)]
mod tests {
    use super::{
        MOCK_REPLACEMENTS, copy_github_variants, is_pipeline_source_path_pub,
        normalize_force_http_engine_hosts, semantic_content_for_path_pub,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn semantic_compare_ignores_dates_comments_and_list_order() {
        let old = "#!name=Rules - [2026-08-11]\n#!date=2026-08-11 10:00:00\nDOMAIN,b.example # source\nDOMAIN,a.example\n";
        let new = "#!name=Rules - [2026-08-12]\n#!date = 2026-08-12 10:00:00\n// generated\nDOMAIN,a.example\nDOMAIN,b.example # refreshed\n";
        assert_eq!(
            semantic_content_for_path_pub(Path::new("rules.list"), old),
            semantic_content_for_path_pub(Path::new("rules.list"), new)
        );
    }

    #[test]
    fn semantic_compare_keeps_real_module_rule_changes() {
        let old =
            "#!name=Rules - [2026-08-11]\n#!date=2026-08-11\n[Rule]\nDOMAIN,a.example,REJECT\n";
        let new =
            "#!name=Rules - [2026-08-12]\n#!date=2026-08-12\n[Rule]\nDOMAIN,b.example,REJECT\n";
        assert_ne!(
            semantic_content_for_path_pub(Path::new("rules.sgmodule"), old),
            semantic_content_for_path_pub(Path::new("rules.sgmodule"), new)
        );
    }

    #[test]
    fn semantic_compare_ignores_module_rule_formatting() {
        let old = "#!name=Rules\n[Rule]\nDOMAIN-SUFFIX,alpha.example,REJECT\n";
        let new =
            "#!name = Rules\n[Rule]\n  DOMAIN-SUFFIX , alpha.example , REJECT  # reformatted\n";
        assert_eq!(
            semantic_content_for_path_pub(Path::new("rules.sgmodule"), old),
            semantic_content_for_path_pub(Path::new("rules.sgmodule"), new)
        );
    }

    #[test]
    fn semantic_compare_ignores_safe_module_section_alignment() {
        let old = "#!name=Module\n[General]\ndns-server = %APPEND% 1.1.1.1, 8.8.8.8\n[Script]\njob = type=http-request,pattern=^https://example.test\n[MITM]\nhostname = %APPEND% api.example.test, www.example.test\n";
        let new = "#!name = Module\n[General]\n dns-server=%APPEND% 1.1.1.1,8.8.8.8 \n[Script]\njob        =    type=http-request,pattern=^https://example.test\n[MITM]\nhostname=%APPEND% api.example.test,www.example.test\n";
        assert_eq!(
            semantic_content_for_path_pub(Path::new("module.sgmodule"), old),
            semantic_content_for_path_pub(Path::new("module.sgmodule"), new)
        );
    }

    #[test]
    fn force_http_engine_hosts_are_merged_and_global_port_wildcards_removed() {
        let input = "#!name=tools\n[General]\nforce-http-engine-hosts = %APPEND% boxjs.com, *.boxjs.com\nforce-http-engine-hosts = %APPEND% *.boxjs.com, script.hub, *:8082\n[MITM]\nhostname = script.hub\n";
        let normalized = normalize_force_http_engine_hosts(input);
        assert_eq!(normalized.matches("force-http-engine-hosts").count(), 1);
        assert!(
            normalized
                .contains("force-http-engine-hosts = %APPEND% boxjs.com, *.boxjs.com, script.hub")
        );
        assert!(!normalized.contains("*:8082"));
    }

    #[test]
    fn semantic_compare_ignores_catalog_timestamps_but_keeps_counts() {
        let old = r#"{"generated":"2026-08-11","modules":[{"name":"PROMAX - [2026-08-11]","date":"2026-08-11","rule_count":10}]}"#;
        let date_only = r#"{"generated":"2026-08-12","modules":[{"name":"PROMAX - [2026-08-12]","date":"2026-08-12","rule_count":10}]}"#;
        let changed = r#"{"generated":"2026-08-12","modules":[{"name":"PROMAX - [2026-08-12]","date":"2026-08-12","rule_count":11}]}"#;
        let path = Path::new("modules/helper/modules_data.json");
        assert_eq!(
            semantic_content_for_path_pub(path, old),
            semantic_content_for_path_pub(path, date_only)
        );
        assert_ne!(
            semantic_content_for_path_pub(path, old),
            semantic_content_for_path_pub(path, changed)
        );
    }

    #[test]
    fn semantic_compare_ignores_embedded_helper_dates() {
        let old = r#"const surgeData = {"items":[{"name":"PROMAX - [2026-08-11]","date":"2026-08-11"}]};"#;
        let new = r#"const surgeData = {"items":[{"name":"PROMAX - [2026-08-12]","date":"2026-08-12"}]};"#;
        let path = Path::new("modules/helper/surge_module_helper.html");
        assert_eq!(
            semantic_content_for_path_pub(path, old),
            semantic_content_for_path_pub(path, new)
        );
    }

    #[test]
    fn source_inputs_are_not_rewritten_as_published_artifacts() {
        assert!(is_pipeline_source_path_pub(Path::new(
            "/repo/rulesets/Sources/Links/AdBlock_sources.list"
        )));
        assert!(is_pipeline_source_path_pub(Path::new(
            "/repo/modules/source/local/YouTube.ADBlock.sgmodule"
        )));
        assert!(!is_pipeline_source_path_pub(Path::new(
            "/repo/modules/surge/head_expanse/PROMAX.sgmodule"
        )));
    }

    #[test]
    fn published_release_urls_are_official_and_stale_mirrors_are_pruned() {
        let proxied =
            "https://ghproxy.net/https://github.com/example/tool/releases/download/v1/a.js";
        let official = MOCK_REPLACEMENTS
            .iter()
            .fold(proxied.to_string(), |value, (re, replacement)| {
                re.replace_all(&value, *replacement).to_string()
            });
        assert_eq!(
            official,
            "https://github.com/example/tool/releases/download/v1/a.js"
        );

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("github-mirror-prune-{id}"));
        let dir = root.join("rulesets/AdBlock");
        let github = dir.join("github");
        fs::create_dir_all(&github).unwrap();
        fs::write(dir.join("Current.list"), "DOMAIN,current.example\n").unwrap();
        fs::write(github.join("Stale.list"), "DOMAIN,stale.example\n").unwrap();
        let promax =
            "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule";
        fs::write(dir.join(promax), "#!name=PROMAX\n").unwrap();
        fs::write(github.join(promax), "#!name=PROMAX GitHub\n").unwrap();

        copy_github_variants(root.to_str().unwrap());

        assert!(github.join("Current.list").is_file());
        assert!(github.join(promax).is_file());
        assert!(!github.join("Stale.list").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
