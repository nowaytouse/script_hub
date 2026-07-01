use regex::Regex;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use once_cell::sync::Lazy;

static RESTRICTED_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    let patterns = vec![
        "github.com", "api.github.com", "*.github.com", "*.api.github.com",
        "raw.githubusercontent.com", "gist.githubusercontent.com",
        "*.objects.githubusercontent.com", "*.githubusercontent.com", "*.github.io",
        "*.apple.com", "*.icloud.com", "*.mzstatic.com", "*.itunes.com",
        "*.facebook.com", "*.instagram.com", "*.twitter.com",
        "*.google.com", "*.google.cn", "*.gmail.com", "*.youtube.com",
        "*.googlevideo.com", "*.gstatic.com", "*.googleapis.com",
        "*.bankofchina.com", "*.icbc.com.cn", "*.ccb.com", "*.cmbchina.com",
        "*.abchina.com", "*.boc.cn", "*.psbc.com", "*.spdb.com.cn", "*.cebbank.com",
        "*.cmbc.com.cn", "*.cib.com.cn", "*.hxb.com.cn", "*.pingan.com",
        "*.bankcomm.com", "*.cgbchina.com.cn", "*.ghbank.com.cn", "*.czbank.com",
        "*.ebank.com",
        "dns.alidns.com", "doh.pub", "dot.pub", "doh.360.cn", "dot.360.cn",
        "dns.baidu.com", "dns.volcengine.com", "alidns.com",
    ];

    patterns.into_iter().map(|pat| {
        let escaped = regex::escape(pat)
            .replace("\\*", ".*")
            .replace("\\?", ".");
        Regex::new(&format!("(?i)^{}$", escaped)).unwrap()
    }).collect()
});

fn is_restricted(domain: &str) -> bool {
    let domain_lower = domain.to_lowercase();
    for re in RESTRICTED_PATTERNS.iter() {
        if re.is_match(&domain_lower) {
            return true;
        }
    }
    false
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
                if val_part.starts_with(t) {
                    tag_with_space = format!("{} ", t);
                    remaining_val = val_part[t.len()..].trim();
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
                let mut check_domain = *d;
                if d.starts_with('-') {
                    check_domain = &d[1..];
                }
                if !is_restricted(check_domain) {
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
            let _ = crate::safe_write_file_internal(path, &final_content, true);
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
        if name.ends_with(".sgmodule") || name.ends_with(".module") {
            if process_file(entry_path, dry_run) {
                if !dry_run {
                    println!("\x1b[0;32m[✓]\x1b[0m MITM Cleaned: {:?}", entry_path);
                }
                modified_count += 1;
            }
        }
    }
    modified_count
}
