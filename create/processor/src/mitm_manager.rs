use std::fs;
use std::path::Path;
use walkdir::WalkDir;

const RESTRICTED_SUFFIXES: &[&str] = &[
    // Script supply-chain and credential-bearing endpoints.
    "github.com",
    "githubusercontent.com",
    "github.io",
    // Financial services are not required by any supported enhancement bundle.
    "bankofchina.com",
    "icbc.com.cn",
    "ccb.com",
    "cmbchina.com",
    "abchina.com",
    "boc.cn",
    "psbc.com",
    "spdb.com.cn",
    "cebbank.com",
    "cmbc.com.cn",
    "cib.com.cn",
    "hxb.com.cn",
    "pingan.com",
    "bankcomm.com",
    "cgbchina.com.cn",
    "ghbank.com.cn",
    "czbank.com",
    "ebank.com",
    // Keep narrower YouTube/Google API hosts usable while excluding accounts.
    "accounts.google.com",
    "gmail.com",
];

const RESTRICTED_EXACT: &[&str] = &[
    "dns.alidns.com",
    "doh.pub",
    "dot.pub",
    "doh.360.cn",
    "dot.360.cn",
    "dns.baidu.com",
    "dns.volcengine.com",
    "alidns.com",
];

pub(crate) fn is_restricted(domain: &str) -> bool {
    if crate::promax::safety::apple_host_pattern_is_critical(domain) {
        return true;
    }

    let domain = domain
        .trim()
        .trim_start_matches("*.")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    RESTRICTED_EXACT.contains(&domain.as_str())
        || RESTRICTED_SUFFIXES
            .iter()
            .any(|suffix| domain == *suffix || domain.ends_with(&format!(".{suffix}")))
}

fn process_file(path: &Path, dry_run: bool) -> bool {
    if !path.exists() || !path.is_file() {
        return false;
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    if !content.contains("[MITM]") {
        return false;
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();
    let mut modified = false;

    // Use a line splitter that preserves the original line ending style if possible,
    // but standard content.lines() is fine.
    for line in lines {
        let stripped = line.trim();
        if stripped.starts_with("hostname") && !stripped.starts_with("hostname-disabled") {
            if !line.contains('=') {
                new_lines.push(line.to_string());
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, '=').collect();
            let prefix = parts[0];
            let val_part = parts[1].trim();

            let mut tag_with_space = String::new();
            let mut remaining_val = val_part;
            for t in &["%APPEND%", "%INSERT%", "%SET%"] {
                if let Some(value) = val_part.strip_prefix(t) {
                    tag_with_space = format!("{} ", t);
                    remaining_val = value.trim();
                    break;
                }
            }

            let domains: Vec<&str> = remaining_val
                .split(',')
                .map(|d| d.trim())
                .filter(|d| !d.is_empty())
                .collect();

            let mut new_domains = Vec::new();
            for d in &domains {
                if d.starts_with('-') {
                    new_domains.push(*d);
                    continue;
                }
                if !is_restricted(d) {
                    new_domains.push(*d);
                }
            }

            if new_domains.len() != domains.len() {
                modified = true;
            }

            let formatted_line = if !new_domains.is_empty() {
                format!("{}= {}{}", prefix, tag_with_space, new_domains.join(", "))
            } else {
                format!("{}= {}", prefix, tag_with_space.trim_end())
            };
            new_lines.push(formatted_line);
        } else {
            new_lines.push(line.to_string());
        }
    }

    if modified {
        if dry_run {
            println!("[DRY RUN] Would modify: {:?}", path);
        } else {
            let mut final_content = new_lines.join("\n");
            if !final_content.ends_with('\n') {
                final_content.push('\n');
            }
            if !crate::safe_write_file_internal(path, &final_content, true) {
                return false;
            }
        }
        true
    } else {
        false
    }
}

pub fn run_mitm_cleanup(directory: &str, dry_run: bool) -> i32 {
    let path = Path::new(directory);
    if !path.exists() {
        return 0;
    }

    let mut modified_count = 0;
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let entry_path = entry.path();
        if entry.file_type().is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if (name.ends_with(".sgmodule") || name.ends_with(".module"))
            && process_file(entry_path, dry_run)
        {
            if !dry_run {
                println!("\x1b[0;32m[✓]\x1b[0m MITM Cleaned: {:?}", entry_path);
            }
            modified_count += 1;
        }
    }
    modified_count
}

#[cfg(test)]
mod tests {
    use super::process_file;
    use std::fs;

    #[test]
    fn cleanup_removes_restricted_mitm_but_keeps_safety_exclusions() {
        let directory =
            std::env::temp_dir().join(format!("script-hub-mitm-cleanup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let module = directory.join("test.sgmodule");
        fs::write(
            &module,
            "#!name=test\n[MITM]\nhostname = %INSERT% gsa.apple.com, gateway.icloud.com, api.github.com, accounts.google.com, online.bankofchina.com, -account.apple.com, -*.apple.com, weatherkit.apple.com, uts-api.itunes.apple.com, youtubei.googleapis.com, *.googlevideo.com, api.example.com\n",
        )
        .unwrap();

        assert!(process_file(&module, false));
        let content = fs::read_to_string(&module).unwrap();
        assert!(!content.contains("%INSERT% gsa.apple.com"));
        assert!(!content.contains("gateway.icloud.com"));
        assert!(!content.contains("api.github.com"));
        assert!(!content.contains("accounts.google.com"));
        assert!(!content.contains("online.bankofchina.com"));
        assert!(content.contains("-account.apple.com"));
        assert!(content.contains("-*.apple.com"));
        assert!(content.contains("weatherkit.apple.com"));
        assert!(content.contains("uts-api.itunes.apple.com"));
        assert!(content.contains("youtubei.googleapis.com"));
        assert!(content.contains("*.googlevideo.com"));
        assert!(content.contains("api.example.com"));
        fs::remove_dir_all(directory).unwrap();
    }
}
