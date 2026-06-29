use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use once_cell::sync::Lazy;

static MOCK_REPLACEMENTS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/reject-200\.txt"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-200.txt"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/blank\.txt"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank.txt"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/reject-dict\.json"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-dict.json"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/reject-img\.gif"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-img.gif"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/blank_dict\.json\.js"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank_dict.json.js"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/blank\.gif"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank.gif"),
        (Regex::new(r#"(?i)https://(?:raw\.githubusercontent\.com|cdn\.jsdelivr\.net/gh)/[^"'\s]+/blank_dict\.json"#).unwrap(), "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank_dict.json"),
        (Regex::new(r#"(?i)https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)"#).unwrap(), "https://cdn.jsdelivr.net/gh/${1}/${2}@${3}/${4}"),
        (Regex::new(r#"(?i)(^|[^/])https://github\.com/([^/]+)/([^/]+)/raw/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)"#).unwrap(), "${1}https://cdn.jsdelivr.net/gh/${2}/${3}@${4}/${5}"),
        (Regex::new(r#"(?i)(^|[^/])https://github\.com/([^/]+)/([^/]+)/releases/download/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)"#).unwrap(), "${1}https://ghproxy.net/https://github.com/${2}/${3}/releases/download/${4}/${5}"),
    ]
});

static REVERSE_CDN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(^|[^/])https://cdn\.jsdelivr\.net/gh/([^/]+)/([^/@]+)@([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)"#).unwrap()
});

pub fn get_semantic_content_pub(text: &str) -> String {
    let mut filtered = Vec::new();
    for line in text.lines() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        if stripped.starts_with('#') || stripped.starts_with("//") {
            continue;
        }
        filtered.push(stripped);
    }
    filtered.join("\n")
}

fn safe_write_file(path: &Path, content: &str) -> std::io::Result<()> {
    if path.exists() {
        if let Ok(old_content) = fs::read_to_string(path) {
            let old_stripped = get_semantic_content_pub(&old_content);
            let new_stripped = get_semantic_content_pub(content);
            if old_stripped == new_stripped {
                return Ok(());
            }
        }
    }
    // Simple write for now; atomic temp writing could be added if critical
    fs::write(path, content)
}

fn is_github_source(path: &Path) -> bool {
    for comp in path.components() {
        if let Some(s) = comp.as_os_str().to_str() {
            if s == "github" {
                return true;
            }
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
        "📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule",
        "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module",
        "📱 Universal Ad-Blocking Rules (PROMAX Lite).module",
    ].iter().cloned().collect();

    for dir in dirs_to_copy {
        if !dir.exists() {
            continue;
        }
        let github_dir = dir.join("github");
        let _ = fs::create_dir_all(&github_dir);

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let file_type = entry.file_type().unwrap();
                let file_name_os = entry.file_name();
                let file_name = file_name_os.to_string_lossy();

                if file_type.is_dir() || file_name == "github" || skip_copy.contains(file_name.as_ref()) {
                    continue;
                }

                if file_name.ends_with(".sgmodule") || file_name.ends_with(".module") || file_name.ends_with(".list") {
                    let src_path = entry.path();
                    let dest_path = github_dir.join(file_name.as_ref());

                    if let Ok(content) = fs::read_to_string(&src_path) {
                        let new_content = REVERSE_CDN_REGEX.replace_all(&content, "${1}https://raw.githubusercontent.com/${2}/${3}/${4}/${5}");
                        let _ = safe_write_file(&dest_path, &new_content);
                    }
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

    let valid_exts: HashSet<&str> = ["sgmodule", "module", "list", "conf", "json", "html", "md"].iter().cloned().collect();
    let mut modified_count = 0;

    for entry in WalkDir::new(directory).into_iter().filter_map(|e| e.ok()) {
        let entry_path = entry.path();

        if entry.file_type().is_dir() {
            continue;
        }

        if entry_path.to_string_lossy().contains(".git") {
            continue;
        }
        if is_github_source(entry_path) {
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
            let mut new_content = content;

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
