use regex::escape;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
struct SingboxRuleset {
    version: i32,
    rules: Vec<HashMap<String, Vec<String>>>,
}

impl SingboxRuleset {
    fn new(rules: HashMap<String, Vec<String>>) -> Self {
        Self {
            version: 1,
            rules: if rules.is_empty() {
                Vec::new()
            } else {
                vec![rules]
            },
        }
    }
}

fn wildcard_to_regex(value: &str) -> String {
    let escaped = escape(value);
    let r = escaped.replace(r"\*", ".*").replace(r"\?", ".");
    format!("^{}$", r)
}

fn rule_fields(line: &str) -> Option<(&str, &str)> {
    let mut fields = line.split(',');
    Some((fields.next()?.trim(), fields.next()?.trim()))
}

fn files_equal(left: &Path, right: &Path) -> std::io::Result<bool> {
    let mut left = fs::File::open(left)?;
    let mut right = fs::File::open(right)?;
    let left_len = left.metadata()?.len();
    if left_len != right.metadata()?.len() {
        return Ok(false);
    }

    let mut remaining = left_len;
    let mut left_buffer = [0_u8; 32 * 1024];
    let mut right_buffer = [0_u8; 32 * 1024];
    while remaining > 0 {
        let chunk_len = remaining.min(left_buffer.len() as u64) as usize;
        left.read_exact(&mut left_buffer[..chunk_len])?;
        right.read_exact(&mut right_buffer[..chunk_len])?;
        if left_buffer[..chunk_len] != right_buffer[..chunk_len] {
            return Ok(false);
        }
        remaining -= chunk_len as u64;
    }
    Ok(true)
}

fn srs_output_dir(list_file: &Path, root_dir: &Path) -> PathBuf {
    if list_file.starts_with(root_dir.join("rulesets/Sources/dns/mapping")) {
        root_dir.join("rulesets/Sources/dns/mapping")
    } else if list_file.starts_with(root_dir.join("rulesets/AdBlock")) {
        root_dir.join("rulesets/AdBlock")
    } else {
        root_dir.join("rulesets/SingBox")
    }
}

fn compile_srs(list_file: &Path, root_dir: &Path, singbox_path: &str) -> bool {
    let file_name = match list_file.file_name().and_then(|f| f.to_str()) {
        Some(f) => f,
        None => return false,
    };
    let name = file_name.trim_end_matches(".list");

    let out_dir = srs_output_dir(list_file, root_dir);

    if !out_dir.exists() && fs::create_dir_all(&out_dir).is_err() {
        return false;
    }

    let srs_file = out_dir.join(format!("{}_Singbox.srs", name));
    let cache_dir = root_dir.join("rulesets/Sources/Links/.cache");
    if !cache_dir.exists() && fs::create_dir_all(&cache_dir).is_err() {
        return false;
    }

    let json_tmp = cache_dir.join(format!("{}.json", name));
    let json_tmp_write = cache_dir.join(format!("{}.json.write", name));

    let content = match fs::read_to_string(list_file) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let mut merged_rules: HashMap<String, HashSet<String>> = HashMap::with_capacity(6);
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
        let Some((rtype, val)) = rule_fields(trimmed) else {
            continue;
        };

        match rtype {
            "DOMAIN" => {
                if let Some(set) = merged_rules.get_mut("domain") {
                    set.insert(val.to_string());
                }
            }
            "DOMAIN-SUFFIX" => {
                if let Some(set) = merged_rules.get_mut("domain_suffix") {
                    set.insert(val.to_string());
                }
            }
            "DOMAIN-KEYWORD" => {
                if let Some(set) = merged_rules.get_mut("domain_keyword") {
                    set.insert(val.to_string());
                }
            }
            "DOMAIN-REGEX" => {
                if let Some(set) = merged_rules.get_mut("domain_regex") {
                    set.insert(val.to_string());
                }
            }
            "DOMAIN-WILDCARD" => {
                if let Some(set) = merged_rules.get_mut("domain_regex") {
                    set.insert(wildcard_to_regex(val));
                }
            }
            "IP-CIDR" | "IP-CIDR6" => {
                if let Some(set) = merged_rules.get_mut("ip_cidr") {
                    set.insert(val.to_string());
                }
            }
            "PROCESS-NAME" => {
                if let Some(set) = merged_rules.get_mut("process_name") {
                    set.insert(val.to_string());
                }
            }
            _ => {}
        }
    }

    let mut final_rules: HashMap<String, Vec<String>> = HashMap::with_capacity(6);
    for (k, v) in merged_rules {
        if !v.is_empty() {
            let mut list: Vec<String> = v.into_iter().collect();
            list.sort();
            final_rules.insert(k, list);
        }
    }

    let srs_json = SingboxRuleset::new(final_rules);

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

    let srs_tmp = srs_file.with_extension("write.srs");
    let output = Command::new(singbox_path)
        .arg("rule-set")
        .arg("compile")
        .arg(&json_tmp)
        .arg("-o")
        .arg(&srs_tmp)
        .output();

    let _ = fs::remove_file(&json_tmp);

    match output {
        Ok(out) => {
            if out.status.success() {
                let changed = !files_equal(&srs_tmp, &srs_file).unwrap_or(false);
                if changed {
                    if fs::rename(&srs_tmp, &srs_file).is_err() {
                        let _ = fs::remove_file(&srs_tmp);
                        return false;
                    }
                } else {
                    let _ = fs::remove_file(&srs_tmp);
                    println!("[INFO] SRS unchanged; publication skipped: {}", name);
                }
                println!("\x1b[0;32m[SUCCESS]\x1b[0m Compiled SRS: {}", name);
                true
            } else {
                let _ = fs::remove_file(&srs_tmp);
                eprintln!(
                    "\x1b[0;31m[ERROR]\x1b[0m Failed to compile {}: {}",
                    name,
                    String::from_utf8_lossy(&out.stderr)
                );
                false
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&srs_tmp);
            eprintln!(
                "\x1b[0;31m[ERROR]\x1b[0m Failed to execute sing-box: {:?}",
                e
            );
            false
        }
    }
}

fn collect_srs_outputs(directory: &Path, outputs: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return crate::verify_missing_directory(directory);
        }
        Err(error) => return Err(format!("cannot list {}: {error}", directory.display())),
    };

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot read {} entry: {error}", directory.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if file_type.is_file() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "srs") {
                outputs.push(path);
            }
        }
    }
    Ok(())
}

fn prune_stale_srs(list_files: &[PathBuf], root_dir: &Path) -> bool {
    let mut expected_srs: HashSet<PathBuf> = HashSet::with_capacity(list_files.len());
    for lf in list_files {
        if let Some(file_name) = lf.file_name().and_then(|f| f.to_str()) {
            let name = file_name.trim_end_matches(".list");
            expected_srs.insert(srs_output_dir(lf, root_dir).join(format!("{}_Singbox.srs", name)));
        }
    }

    let singbox_dir = root_dir.join("rulesets/SingBox");
    let dns_dir = root_dir.join("rulesets/Sources/dns/mapping");
    let adblock_dir = root_dir.join("rulesets/AdBlock");
    let mut outputs = Vec::new();
    for directory in [&singbox_dir, &dns_dir, &adblock_dir] {
        if let Err(error) = collect_srs_outputs(directory, &mut outputs) {
            eprintln!("\x1b[0;31m[ERROR]\x1b[0m Skipping stale SRS pruning: {error}");
            return false;
        }
    }

    let mut success = true;
    for path in outputs {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        let is_geoip = path.parent() == Some(singbox_dir.as_path())
            && name.starts_with("GeoIP_")
            && name.ends_with(".srs");
        if !expected_srs.contains(&path) && !is_geoip {
            match fs::remove_file(&path) {
                Ok(()) => println!("\x1b[0;33m[WARN]\x1b[0m Pruned stale SRS: {name}"),
                Err(error) => {
                    eprintln!(
                        "\x1b[0;31m[ERROR]\x1b[0m Failed to prune stale SRS {}: {error}",
                        path.display()
                    );
                    success = false;
                }
            }
        }
    }
    success
}

pub fn run_srs_generator(root_path: &str, singbox_path: &str) -> bool {
    let root_dir = Path::new(root_path);
    if singbox_path.is_empty() {
        eprintln!(
            "\x1b[0;31m[ERROR]\x1b[0m sing-box binary path is empty. Skipping SRS generation."
        );
        return false;
    }

    let source_dirs = [
        (root_dir.join("rulesets/RULE-SET"), true),
        (root_dir.join("rulesets/Sources/dns/mapping"), false),
        (root_dir.join("rulesets/AdBlock"), true),
    ];

    let mut list_files: Vec<PathBuf> = Vec::new();

    for (dir, required) in source_dirs {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => {
                if let Err(error) = crate::verify_missing_directory(&dir) {
                    eprintln!("[ERROR] Cannot enumerate optional SRS sources: {error}");
                    return false;
                }
                continue;
            }
            Err(error) => {
                eprintln!(
                    "\x1b[0;31m[ERROR]\x1b[0m Cannot enumerate SRS sources in {}: {error}",
                    dir.display()
                );
                return false;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    eprintln!(
                        "\x1b[0;31m[ERROR]\x1b[0m Cannot read SRS source entry in {}: {error}",
                        dir.display()
                    );
                    return false;
                }
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    eprintln!(
                        "\x1b[0;31m[ERROR]\x1b[0m Cannot inspect SRS source {}: {error}",
                        entry.path().display()
                    );
                    return false;
                }
            };
            if file_type.is_file() {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == "list")
                {
                    list_files.push(path);
                }
            }
        }
    }

    // Add extra files
    let extra_source_files = [
        root_dir.join("rulesets/AdBlock/reject-drop.list"),
        root_dir.join("rulesets/AdBlock/reject-no-drop.list"),
        root_dir.join("rulesets/Sources/custom/substore.list"),
    ];

    for path in extra_source_files {
        if path.is_file() {
            list_files.push(path);
        }
    }

    list_files.sort();
    list_files.dedup();
    if list_files.is_empty() {
        eprintln!(
            "\x1b[0;31m[ERROR]\x1b[0m No SRS source lists found; skipping generation and pruning."
        );
        return false;
    }

    let mut success = true;
    for lf in &list_files {
        success &= compile_srs(lf, root_dir, singbox_path);
    }

    if !success {
        eprintln!(
            "\x1b[0;31m[ERROR]\x1b[0m Skipping stale SRS pruning after a compilation failure."
        );
        return false;
    }
    prune_stale_srs(&list_files, root_dir)
}

#[cfg(test)]
mod tests {
    use super::{SingboxRuleset, files_equal, prune_stale_srs, rule_fields, run_srs_generator};
    use std::collections::HashMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("script-hub-srs-{name}-{unique}"))
    }

    #[test]
    fn empty_rules_serialize_as_no_matchers_instead_of_an_empty_matcher() {
        let ruleset = SingboxRuleset::new(HashMap::new());

        assert_eq!(
            serde_json::to_value(ruleset).unwrap(),
            serde_json::json!({"version": 1, "rules": []})
        );
    }

    #[test]
    fn nonempty_rules_keep_their_single_matcher() {
        let ruleset = SingboxRuleset::new(HashMap::from([(
            "domain".to_string(),
            vec!["example.com".to_string()],
        )]));

        assert_eq!(
            serde_json::to_value(ruleset).unwrap(),
            serde_json::json!({"version": 1, "rules": [{"domain": ["example.com"]}]})
        );
    }

    #[test]
    fn rule_fields_borrows_the_first_two_fields_without_changing_csv_semantics() {
        assert_eq!(
            rule_fields(" DOMAIN-SUFFIX , example.com , REJECT "),
            Some(("DOMAIN-SUFFIX", "example.com"))
        );
        assert_eq!(rule_fields("DOMAIN-SUFFIX"), None);
    }

    #[test]
    fn files_equal_compares_content_with_equal_and_different_sizes() {
        let root = fixture_root("file-compare");
        fs::create_dir_all(&root).unwrap();
        let left = root.join("left.srs");
        let right = root.join("right.srs");
        fs::write(&left, b"stable output").unwrap();
        fs::write(&right, b"stable output").unwrap();
        assert!(files_equal(&left, &right).unwrap());

        fs::write(&right, b"stable outpuX").unwrap();
        assert!(!files_equal(&left, &right).unwrap());

        fs::write(&right, b"changed").unwrap();
        assert!(!files_equal(&left, &right).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_srs_inventory_preserves_existing_outputs() {
        let root = fixture_root("empty-inventory");
        let output = root.join("rulesets/SingBox/Old_Singbox.srs");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, b"last known good").unwrap();

        assert!(!run_srs_generator(
            root.to_str().unwrap(),
            "unused-sing-box"
        ));
        assert!(output.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_source_inventory_preserves_existing_outputs() {
        let root = fixture_root("source-error");
        fs::create_dir_all(root.join("rulesets")).unwrap();
        fs::write(root.join("rulesets/RULE-SET"), "not a directory").unwrap();
        let output = root.join("rulesets/SingBox/Old_Singbox.srs");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, b"last known good").unwrap();

        assert!(!run_srs_generator(
            root.to_str().unwrap(),
            "unused-sing-box"
        ));
        assert!(output.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_compilation_does_not_prune_existing_outputs() {
        let root = fixture_root("compile-error");
        let sources = root.join("rulesets/RULE-SET");
        fs::create_dir_all(&sources).unwrap();
        fs::create_dir_all(root.join("rulesets/AdBlock")).unwrap();
        fs::write(sources.join("Current.list"), "DOMAIN,current.example\n").unwrap();
        let output = root.join("rulesets/SingBox/Old_Singbox.srs");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, b"last known good").unwrap();

        assert!(!run_srs_generator(
            root.to_str().unwrap(),
            root.join("missing-sing-box").to_str().unwrap()
        ));
        assert!(output.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_pruning_waits_for_both_directory_inventories() {
        let root = fixture_root("prune-error");
        let singbox_dir = root.join("rulesets/SingBox");
        fs::create_dir_all(&singbox_dir).unwrap();
        let stale_output = singbox_dir.join("Old_Singbox.srs");
        fs::write(&stale_output, b"last known good").unwrap();
        let dns_directory = root.join("rulesets/Sources/dns");
        fs::create_dir_all(dns_directory.parent().unwrap()).unwrap();
        fs::write(&dns_directory, "not a directory").unwrap();

        assert!(!prune_stale_srs(&[], &root));
        assert!(stale_output.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_pruning_keeps_current_and_geoip_outputs() {
        let root = fixture_root("prune-success");
        let singbox_dir = root.join("rulesets/SingBox");
        let dns_dir = root.join("rulesets/Sources/dns/mapping");
        fs::create_dir_all(&singbox_dir).unwrap();
        fs::create_dir_all(&dns_dir).unwrap();
        let current = singbox_dir.join("Current_Singbox.srs");
        let stale = singbox_dir.join("Old_Singbox.srs");
        let geoip = singbox_dir.join("GeoIP_CN.srs");
        let stale_dns = dns_dir.join("Old_Singbox.srs");
        for path in [&current, &stale, &geoip, &stale_dns] {
            fs::write(path, b"fixture").unwrap();
        }
        let list_files = [root.join("rulesets/RULE-SET/Current.list")];

        assert!(prune_stale_srs(&list_files, &root));
        assert!(current.exists());
        assert!(geoip.exists());
        assert!(!stale.exists());
        assert!(!stale_dns.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_pruning_matches_output_paths_and_cleans_adblock_outputs() {
        let root = fixture_root("prune-output-paths");
        let singbox_dir = root.join("rulesets/SingBox");
        let dns_dir = root.join("rulesets/Sources/dns/mapping");
        let adblock_dir = root.join("rulesets/AdBlock");
        for directory in [&singbox_dir, &dns_dir, &adblock_dir] {
            fs::create_dir_all(directory).unwrap();
        }

        let current = singbox_dir.join("Shared_Singbox.srs");
        let stale_dns = dns_dir.join("Shared_Singbox.srs");
        let stale_adblock = adblock_dir.join("Shared_Singbox.srs");
        let stale_adblock_source = adblock_dir.join("Old_Singbox.srs");
        let geoip = singbox_dir.join("GeoIP_CN.srs");
        for path in [
            &current,
            &stale_dns,
            &stale_adblock,
            &stale_adblock_source,
            &geoip,
        ] {
            fs::write(path, b"fixture").unwrap();
        }
        let list_files = [root.join("rulesets/RULE-SET/Shared.list")];

        assert!(prune_stale_srs(&list_files, &root));
        assert!(current.exists());
        assert!(geoip.exists());
        assert!(!stale_dns.exists());
        assert!(!stale_adblock.exists());
        assert!(!stale_adblock_source.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
