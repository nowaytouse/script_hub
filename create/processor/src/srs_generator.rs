use regex::escape;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
struct SingboxRuleset {
    version: i32,
    rules: Vec<HashMap<String, Vec<String>>>,
}

fn wildcard_to_regex(value: &str) -> String {
    let escaped = escape(value);
    let r = escaped.replace(r"\*", ".*").replace(r"\?", ".");
    format!("^{}$", r)
}

fn compile_srs(list_file: &Path, root_dir: &Path, singbox_path: &str) -> bool {
    let file_name = match list_file.file_name().and_then(|f| f.to_str()) {
        Some(f) => f,
        None => return false,
    };
    let name = file_name.trim_end_matches(".list");

    let mut out_dir = root_dir.join("rulesets/SingBox");
    let path_str = list_file.to_string_lossy();
    if path_str.contains("dns/mapping") || path_str.contains("/dns/") {
        out_dir = root_dir.join("rulesets/Sources/dns/mapping");
    } else if path_str.contains("rulesets/AdBlock") {
        out_dir = root_dir.join("rulesets/AdBlock");
    }

    if !out_dir.exists() {
        if fs::create_dir_all(&out_dir).is_err() {
            return false;
        }
    }

    let srs_file = out_dir.join(format!("{}_Singbox.srs", name));
    let cache_dir = root_dir.join("rulesets/Sources/Links/.cache");
    if !cache_dir.exists() {
        if fs::create_dir_all(&cache_dir).is_err() {
            return false;
        }
    }

    let json_tmp = cache_dir.join(format!("{}.json", name));
    let json_tmp_write = cache_dir.join(format!("{}.json.write", name));

    let content = match fs::read_to_string(list_file) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let mut merged_rules: HashMap<String, HashSet<String>> = HashMap::new();
    merged_rules.insert("domain".to_string(), HashSet::new());
    merged_rules.insert("domain_suffix".to_string(), HashSet::new());
    merged_rules.insert("domain_keyword".to_string(), HashSet::new());
    merged_rules.insert("domain_regex".to_string(), HashSet::new());
    merged_rules.insert("ip_cidr".to_string(), HashSet::new());
    merged_rules.insert("process_name".to_string(), HashSet::new());

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let rtype = parts[0].trim();
        let val = parts[1].trim().to_string();

        match rtype {
            "DOMAIN" => {
                if let Some(set) = merged_rules.get_mut("domain") {
                    set.insert(val);
                }
            }
            "DOMAIN-SUFFIX" => {
                if let Some(set) = merged_rules.get_mut("domain_suffix") {
                    set.insert(val);
                }
            }
            "DOMAIN-KEYWORD" => {
                if let Some(set) = merged_rules.get_mut("domain_keyword") {
                    set.insert(val);
                }
            }
            "DOMAIN-REGEX" => {
                if let Some(set) = merged_rules.get_mut("domain_regex") {
                    set.insert(val);
                }
            }
            "DOMAIN-WILDCARD" => {
                if let Some(set) = merged_rules.get_mut("domain_regex") {
                    set.insert(wildcard_to_regex(&val));
                }
            }
            "IP-CIDR" | "IP-CIDR6" => {
                if let Some(set) = merged_rules.get_mut("ip_cidr") {
                    set.insert(val);
                }
            }
            "PROCESS-NAME" => {
                if let Some(set) = merged_rules.get_mut("process_name") {
                    set.insert(val);
                }
            }
            _ => {}
        }
    }

    let mut final_rules: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in merged_rules {
        if !v.is_empty() {
            let mut list: Vec<String> = v.into_iter().collect();
            list.sort();
            final_rules.insert(k, list);
        }
    }

    let srs_json = SingboxRuleset {
        version: 1,
        rules: vec![final_rules],
    };

    let data = match serde_json::to_string(&srs_json) {
        Ok(d) => d,
        Err(_) => return false,
    };

    if fs::write(&json_tmp_write, data).is_err() {
        return false;
    }
    if fs::rename(&json_tmp_write, &json_tmp).is_err() {
        let _ = fs::remove_file(&json_tmp_write);
        return false;
    }

    let output = Command::new(singbox_path)
        .arg("rule-set")
        .arg("compile")
        .arg(&json_tmp)
        .arg("-o")
        .arg(&srs_file)
        .output();

    let _ = fs::remove_file(&json_tmp);

    match output {
        Ok(out) => {
            if out.status.success() {
                println!("\x1b[0;32m[SUCCESS]\x1b[0m Compiled SRS: {}", name);
                true
            } else {
                eprintln!(
                    "\x1b[0;31m[ERROR]\x1b[0m Failed to compile {}: {}",
                    name,
                    String::from_utf8_lossy(&out.stderr)
                );
                false
            }
        }
        Err(e) => {
            eprintln!(
                "\x1b[0;31m[ERROR]\x1b[0m Failed to execute sing-box: {:?}",
                e
            );
            false
        }
    }
}

fn prune_stale_srs(list_files: &[PathBuf], root_dir: &Path) {
    let mut expected_srs: HashSet<String> = HashSet::new();
    for lf in list_files {
        if let Some(file_name) = lf.file_name().and_then(|f| f.to_str()) {
            let name = file_name.trim_end_matches(".list");
            expected_srs.insert(format!("{}_Singbox.srs", name));
        }
    }

    let singbox_dir = root_dir.join("rulesets/SingBox");
    if let Ok(entries) = fs::read_dir(&singbox_dir) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.starts_with("GeoIP_") && name.ends_with(".srs") {
                    expected_srs.insert(name);
                }
            }
        }
    }

    // Prune SINGBOX_DIR
    if let Ok(entries) = fs::read_dir(&singbox_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.ends_with(".srs") && !expected_srs.contains(&name) {
                        if fs::remove_file(&path).is_ok() {
                            println!("\x1b[0;33m[WARN]\x1b[0m Pruned stale SRS: {}", name);
                        } else {
                            eprintln!(
                                "\x1b[0;31m[ERROR]\x1b[0m Failed to prune stale SRS: {}",
                                name
                            );
                        }
                    }
                }
            }
        }
    }

    // Prune SINGBOX_DNS_DIR
    let dns_dir = root_dir.join("rulesets/Sources/dns/mapping");
    if dns_dir.exists() {
        if let Ok(entries) = fs::read_dir(&dns_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if name.ends_with(".srs") && !expected_srs.contains(&name) {
                            if fs::remove_file(&path).is_ok() {
                                println!("\x1b[0;33m[WARN]\x1b[0m Pruned stale DNS SRS: {}", name);
                            } else {
                                eprintln!(
                                    "\x1b[0;31m[ERROR]\x1b[0m Failed to prune stale DNS SRS: {}",
                                    name
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn run_srs_generator(root_path: &str, singbox_path: &str) -> bool {
    let root_dir = Path::new(root_path);
    if singbox_path.is_empty() {
        eprintln!(
            "\x1b[0;31m[ERROR]\x1b[0m sing-box binary path is empty. Skipping SRS generation."
        );
        return false;
    }

    let source_dirs = vec![
        root_dir.join("rulesets/RULE-SET"),
        root_dir.join("rulesets/Sources/dns/mapping"),
        root_dir.join("rulesets/AdBlock"),
    ];

    let mut list_files: Vec<PathBuf> = Vec::new();

    for dir in source_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "list" {
                            list_files.push(path);
                        }
                    }
                }
            }
        }
    }

    // Add extra files
    let extra_source_files = vec![
        root_dir.join("rulesets/AdBlock/reject-drop.list"),
        root_dir.join("rulesets/AdBlock/reject-no-drop.list"),
        root_dir.join("rulesets/Sources/custom/substore.list"),
    ];

    for path in extra_source_files {
        if path.exists() {
            list_files.push(path);
        }
    }

    // Deduplicate
    let mut unique_files = HashSet::new();
    let mut deduped_list = Vec::new();
    for f in list_files {
        if unique_files.insert(f.clone()) {
            deduped_list.push(f);
        }
    }

    let mut success = true;
    for lf in &deduped_list {
        success &= compile_srs(lf, root_dir, singbox_path);
    }

    prune_stale_srs(&deduped_list, root_dir);
    success
}
