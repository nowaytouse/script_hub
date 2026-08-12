use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

static PORT_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IN-PORT|DEST-PORT|SRC-PORT)").unwrap());

static VERSION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"#!version=.*").unwrap());

pub fn sync_ports(ports_source: &str, firewall_modules: &[&str], execute: bool, version_str: &str) {
    println!("--- Firewall Port Sync (English Script Interface) ---");

    let source_path = Path::new(ports_source);
    if !source_path.exists() {
        println!(
            "\x1b[0;33m[WARN]\x1b[0m Port rules source not found: {}",
            ports_source
        );
        return;
    }

    let source_content = match fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(e) => {
            println!(
                "\x1b[0;31m[ERROR]\x1b[0m Failed to read ports source: {:?}",
                e
            );
            return;
        }
    };

    let mut port_rules = Vec::new();
    let mut invalid_rules = Vec::new();

    for line in source_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if PORT_PATTERN.is_match(trimmed) {
            let parts: Vec<&str> = trimmed.split(',').collect();
            if parts.len() < 3 {
                invalid_rules.push(trimmed.to_string());
                println!(
                    "\x1b[0;33m[WARN]\x1b[0m   Invalid rule (missing action): {}",
                    trimmed
                );
            } else {
                port_rules.push(trimmed.to_string());
            }
        }
    }

    if !invalid_rules.is_empty() {
        println!(
            "\x1b[0;33m[WARN]\x1b[0m Skipping {} invalid rules (missing action)",
            invalid_rules.len()
        );
        for rule in &invalid_rules {
            println!("\x1b[0;33m[WARN]\x1b[0m   - {}", rule);
        }
    }

    if port_rules.is_empty() {
        println!("\x1b[0;33m[WARN]\x1b[0m No port rules found in source.");
        return;
    }

    println!(
        "\x1b[0;34m[INFO]\x1b[0m Extracted {} valid rules from source.",
        port_rules.len()
    );

    let mut managed_keys = HashMap::new();
    for rule in &port_rules {
        let parts: Vec<&str> = rule.split(',').collect();
        if parts.len() >= 2 {
            let key = format!(
                "{},{}",
                parts[0].trim().to_uppercase(),
                parts[1].trim().to_uppercase()
            );
            managed_keys.insert(key, true);
        }
    }

    for module_path_str in firewall_modules {
        let module_path = Path::new(module_path_str);
        if !module_path.exists() {
            println!(
                "\x1b[0;33m[WARN]\x1b[0m Firewall module not found: {}",
                module_path_str
            );
            continue;
        }

        let module_content = match fs::read_to_string(module_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut new_module_content = Vec::new();
        let start_marker = "# --- SYNCED PORT RULES START ---";
        let end_marker = "# --- SYNCED PORT RULES END ---";
        let mut skip = false;
        let mut in_rule_section = false;

        for line in module_content.lines() {
            let stripped = line.trim();
            if stripped.starts_with('[') && stripped.ends_with(']') {
                in_rule_section = stripped.to_lowercase() == "[rule]";
            }

            if line.contains(start_marker) {
                new_module_content.push(line.to_string());
                new_module_content.push("# Automated update from ports source".to_string());

                let mut sorted_rules = port_rules.clone();
                sorted_rules.sort();
                for rule in sorted_rules {
                    new_module_content.push(rule);
                }
                skip = true;
                continue;
            }

            if line.contains(end_marker) {
                skip = false;
                new_module_content.push(line.to_string());
                continue;
            }

            if !skip {
                if in_rule_section && !stripped.is_empty() && !stripped.starts_with('#') {
                    let parts: Vec<&str> = stripped.split(',').collect();
                    if parts.len() >= 2 {
                        let key = format!(
                            "{},{}",
                            parts[0].trim().to_uppercase(),
                            parts[1].trim().to_uppercase()
                        );
                        if managed_keys.contains_key(&key) {
                            continue;
                        }
                    }
                }
                new_module_content.push(line.to_string());
            }
        }

        let mut final_content = new_module_content.join("\n");
        if !final_content.ends_with('\n') {
            final_content.push('\n');
        }

        let file_name = module_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        if execute {
            let new_version = format!("#!version={}", version_str);
            final_content = VERSION_RE
                .replace_all(&final_content, new_version.as_str())
                .to_string();
            let success = crate::safe_write_file_internal(module_path, &final_content, true);
            if success {
                println!(
                    "\x1b[0;32m[✓]\x1b[0m Firewall module updated successfully: {}",
                    file_name
                );
            }
        } else {
            println!(
                "\x1b[0;34m[INFO]\x1b[0m Dry Run: Use execute=true to apply changes to {}.",
                file_name
            );
        }
    }
}
