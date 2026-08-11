use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Write};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::Path;
use regex::Regex;
use once_cell::sync::Lazy;

#[allow(dead_code)]
static DOMAIN_RULE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^DOMAIN,([^,]+)$").unwrap());
#[allow(dead_code)]
static DOMAIN_SUFFIX_RULE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^DOMAIN-SUFFIX,([^,]+)$").unwrap());
#[allow(dead_code)]
static VALID_DOMAIN: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\*\.)?[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$").unwrap());
static VALID_IPV4_CIDR: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\d{1,3}\.){3}\d{1,3}(/\d{1,2})?$").unwrap());
static VALID_IPV6_CIDR: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([0-9a-fA-F:]+)(/\d{1,3})?$").unwrap());

static M1: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\(\^\|\.\)(.+?)-\.\+\.").unwrap());
static M2: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?:\(\^\|\.\)|\^)([a-zA-Z0-9][-a-zA-Z0-9.]*)$").unwrap());
static M3: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\.\+(.+)$").unwrap());

fn is_invalid_domain_regex(payload: &str) -> bool {
    let val = payload.trim();
    val.len() < 2 || ["$", ",", "-", ".", "2", "6", "]", "["].contains(&val)
}

fn is_invalid_url_regex(payload: &str) -> bool {
    let val = payload.trim().to_lowercase();
    val.len() < 3 || ["https:", "http:", "https", "http", "^https?:", "^https?://", "^https?:\\/\\/"].contains(&val.as_str())
}

fn convert_domain_regex_for_surge(rule: &str) -> Option<String> {
    if !rule.to_uppercase().starts_with("DOMAIN-REGEX,") {
        return None;
    }
    let parts: Vec<&str> = rule.splitn(2, ',').collect();
    let payload = parts[1].trim().trim_matches('"');
    if is_invalid_domain_regex(payload) {
        return None;
    }
    let literal = payload.replace("\\.", ".");
    if let Some(caps) = M1.captures(&literal) {
        return Some(format!("DOMAIN-KEYWORD,{}", &caps[1]));
    }
    if let Some(caps) = M2.captures(&literal) {
        return Some(format!("DOMAIN-KEYWORD,{}", &caps[1]));
    }
    if let Some(caps) = M3.captures(&literal) {
        let kw = &caps[1];
        if kw.len() >= 3 {
            return Some(format!("DOMAIN-KEYWORD,{}", kw));
        }
    }
    None
}

fn format_url_regex_for_surge(payload: &str) -> String {
    if payload.contains(',') || payload.contains(' ') {
        format!("URL-REGEX,\"{}\"", payload)
    } else {
        format!("URL-REGEX,{}", payload)
    }
}

fn validate_rule_value(rule_type: &str, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    match rule_type {
        "DOMAIN" | "DOMAIN-SUFFIX" => {
            let val = value.trim_start_matches('.');
            val.contains('.') || val.len() > 2
        }
        "DOMAIN-KEYWORD" => {
            if value.len() < 2 {
                return false;
            }
            for c in ['<', '>', '{', '}', '(', ')'] {
                if value.contains(c) {
                    return false;
                }
            }
            true
        }
        "IP-CIDR" => VALID_IPV4_CIDR.is_match(value),
        "IP-CIDR6" => VALID_IPV6_CIDR.is_match(value),
        "DOMAIN-REGEX" => {
            if is_invalid_domain_regex(value) {
                return false;
            }
            Regex::new(value).is_ok()
        }
        "URL-REGEX" => {
            if is_invalid_url_regex(value) {
                return false;
            }
            Regex::new(value).is_ok()
        }
        _ => true,
    }
}

pub fn strip_inline_comment(line: &str) -> String {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") {
        return String::new();
    }
    // Extract head
    let head = if !line.contains(',') {
        line.to_uppercase()
    } else {
        line.splitn(2, ',').next().unwrap().to_uppercase()
    };
    
    let payload_safe = ["DOMAIN-REGEX", "URL-REGEX", "USER-AGENT", "PROCESS-NAME"];
    if payload_safe.contains(&head.as_str()) {
        return line.to_string();
    }
    
    let mut cleaned = line;
    if let Some(idx) = line.find("//") {
        cleaned = &line[..idx];
    }
    if let Some(idx) = cleaned.find('#') {
        cleaned = &cleaned[..idx];
    }
    cleaned.trim().to_string()
}

fn normalize_rule(line: &str) -> Option<String> {
    let line = strip_inline_comment(&line);
    if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
        return None;
    }
    if !line.contains(',') {
        return None;
    }
    let parts: Vec<&str> = line.splitn(2, ',').collect();
    let rule_type = parts[0].trim().to_uppercase();
    let mut rule_value = parts[1].trim().to_string();
    
    if rule_type.starts_with("IP-") {
        if let Some(idx) = rule_value.find(',') {
            rule_value = rule_value[..idx].trim().to_string();
        }
    }
    
    let valid_types = [
        "DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "IP-CIDR", "IP-CIDR6",
        "DOMAIN-REGEX", "GEOIP", "USER-AGENT", "URL-REGEX", "PROCESS-NAME"
    ];
    if !valid_types.contains(&rule_type.as_str()) {
        return None;
    }
    
    if rule_value.starts_with('"') && rule_value.ends_with('"') && rule_value.len() >= 2 {
        rule_value = rule_value[1..rule_value.len()-1].to_string();
    }
    
    if !validate_rule_value(&rule_type, &rule_value) {
        return None;
    }
    
    if rule_type == "URL-REGEX" {
        return Some(format_url_regex_for_surge(&rule_value));
    }
    
    Some(format!("{},{}", rule_type, rule_value))
}

fn clean_rule(rule: &str, ruleset_name: &str, conflict_domains: &[String], skip_conflict: bool) -> Option<String> {
    let mut r = rule.trim().to_string();
    if r.is_empty() || r.starts_with('#') {
        return None;
    }
    if r == "payload:" || r == "payload" {
        return None;
    }
    if r.starts_with('-') {
        r = r[1..].trim().trim_matches(|c| c == '\'' || c == '"').to_string();
        if r.is_empty() {
            return None;
        }
    }
    
    if !r.contains(',') {
        if r.contains(':') || (r.split('.').count() == 4 && r.split('.').next().unwrap().chars().all(|c| c.is_ascii_digit())) {
            if r.contains('/') {
                if r.contains(':') {
                    r = format!("IP-CIDR6,{}", r);
                } else {
                    r = format!("IP-CIDR,{}", r);
                }
            } else {
                if r.contains(':') {
                    r = format!("IP-CIDR6,{}/128", r);
                } else {
                    r = format!("IP-CIDR,{}/32", r);
                }
            }
        } else {
            if r.starts_with('.') {
                r = format!("DOMAIN-SUFFIX,{}", &r[1..]);
            } else if r.starts_with("+.") {
                r = format!("DOMAIN-SUFFIX,{}", &r[2..]);
            } else if r.starts_with('+') {
                r = format!("DOMAIN-SUFFIX,{}", &r[1..]);
            } else {
                r = format!("DOMAIN,{}", r);
            }
        }
    }
    
    let mut normalized = normalize_rule(&r)?;
    
    if normalized.starts_with("DOMAIN-REGEX,") {
        normalized = convert_domain_regex_for_surge(&normalized)?;
    }
    
    if !skip_conflict {
        for dom in conflict_domains {
            let suffix = format!(",{}", dom);
            if normalized.ends_with(&suffix) {
                return None;
            }
        }
    }
    
    if ruleset_name == "Direct" {
        let r_lower = normalized.to_lowercase();
        let mandatory_proxy = [
            "google", "gstatic", "gmail", "ggpht", "youtube", "ytimg",
            "facebook", "fbcdn", "instagram", "twitter", "twimg", "t.co",
            "telegram", "netflix", "nflxvideo", "nflxext", "disney", "github",
            "akamai", "fastly", "cloudflare", "browserleaks",
            "byteoversea", "tiktok", "tiktokv", "tiktokcdn", "tiktokeu",
            "tiktokw", "muscdn", "tiktokrow", "lark.com"
        ];
        for kw in &mandatory_proxy {
            if r_lower.contains(kw) {
                return None;
            }
        }
    }
    
    Some(normalized)
}

fn generate_header(name: &str, policy: &str, node: &str, desc: &str, count: usize) -> String {
    let mut header = vec![
        format!("# Ruleset: {}", name),
        format!("# Policy: {}", policy),
    ];
    if !node.is_empty() {
        header.push(format!("# Node: {}", node));
    }
    header.push(format!("# Description: {}", desc));
    header.push(format!("# Rules: {}", count));
    
    header.join("\n") + "\n"
}

fn semantic_rules_match(path: &Path, new_rules: &[String]) -> bool {
    if let Ok(content) = fs::read_to_string(path) {
        let mut old_rules = Vec::new();
        for line in content.lines() {
            let stripped = line.trim();
            if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with("//") {
                continue;
            }
            old_rules.push(stripped.to_string());
        }
        if old_rules.len() != new_rules.len() {
            return false;
        }
        for (i, r) in old_rules.iter().enumerate() {
            if r != &new_rules[i] {
                return false;
            }
        }
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn process_ruleset_ffi(
    name: *const c_char,
    target_dir: *const c_char,
    conflict_domains: *const c_char,
    skip_conflict: bool,
    policy: *const c_char,
    node: *const c_char,
    desc: *const c_char,
    local_paths: *const c_char,
    remote_content: *const c_char,
) -> bool {
    if name.is_null() || target_dir.is_null() || conflict_domains.is_null()
        || policy.is_null() || node.is_null() || desc.is_null()
        || local_paths.is_null() || remote_content.is_null() {
        return false;
    }
    
    let name_str = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let target_dir_str = unsafe { CStr::from_ptr(target_dir) }.to_string_lossy();
    let conflict_domains_str = unsafe { CStr::from_ptr(conflict_domains) }.to_string_lossy();
    let policy_str = unsafe { CStr::from_ptr(policy) }.to_string_lossy();
    let node_str = unsafe { CStr::from_ptr(node) }.to_string_lossy();
    let desc_str = unsafe { CStr::from_ptr(desc) }.to_string_lossy();
    let local_paths_str = unsafe { CStr::from_ptr(local_paths) }.to_string_lossy();
    let remote_content_str = unsafe { CStr::from_ptr(remote_content) }.to_string_lossy();

    let conflict_doms: Vec<String> = conflict_domains_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut all_rules = HashSet::new();
    
    for path_str in local_paths_str.split('\n') {
        let trimmed_path = path_str.trim();
        if trimmed_path.is_empty() {
            continue;
        }
        if let Ok(content) = fs::read_to_string(trimmed_path) {
            for line in content.lines() {
                if let Some(cleaned) = clean_rule(line, &name_str, &conflict_doms, skip_conflict) {
                    all_rules.insert(cleaned);
                }
            }
        }
    }

    for line in remote_content_str.lines() {
        if let Some(cleaned) = clean_rule(line, &name_str, &conflict_doms, skip_conflict) {
            all_rules.insert(cleaned);
        }
    }
    
    let mut ip_rules = Vec::new();
    let mut non_ip_rules = Vec::new();
    
    for rule in all_rules {
        if name_str != "Direct" && (rule.starts_with("IP-CIDR,") || rule.starts_with("IP-CIDR6,") || rule.starts_with("IP-ASN,") || rule.starts_with("GEOIP,")) {
            ip_rules.push(rule);
        } else {
            non_ip_rules.push(rule);
        }
    }
    
    ip_rules.sort();
    non_ip_rules.sort();
    
    let target_file_path = Path::new(target_dir_str.as_ref()).join(format!("{}.list", name_str));
    let target_ip_file_path = Path::new(target_dir_str.as_ref()).join(format!("{}_ip.list", name_str));
    
    if !non_ip_rules.is_empty() {
        if semantic_rules_match(&target_file_path, &non_ip_rules) {
            println!("\x1b[0;34m[INFO]\x1b[0m Skipping write for {}: No semantic changes detected.", target_file_path.file_name().unwrap_or_default().to_string_lossy());
        } else {
            if let Ok(mut file) = File::create(&target_file_path) {
                let header_str = generate_header(&name_str, &policy_str, &node_str, &desc_str, non_ip_rules.len());
                let _ = write!(file, "{}", header_str);
                for r in &non_ip_rules {
                    let _ = writeln!(file, "{}", r);
                }
            } else {
                return false;
            }
        }
    } else if target_file_path.exists() {
        let _ = fs::remove_file(&target_file_path);
    }
    
    if !ip_rules.is_empty() {
        if semantic_rules_match(&target_ip_file_path, &ip_rules) {
            println!("\x1b[0;34m[INFO]\x1b[0m Skipping write for {}: No semantic changes detected.", target_ip_file_path.file_name().unwrap_or_default().to_string_lossy());
        } else {
            if let Ok(mut file) = File::create(&target_ip_file_path) {
                let ip_name = format!("{}_ip", name_str);
                let header_str = generate_header(&ip_name, &policy_str, &node_str, &desc_str, ip_rules.len());
                let _ = write!(file, "{}", header_str);
                for r in &ip_rules {
                    let _ = writeln!(file, "{}", r);
                }
            } else {
                return false;
            }
        }
    } else if target_ip_file_path.exists() {
        let _ = fs::remove_file(&target_ip_file_path);
    }

    true
}

#[unsafe(no_mangle)]
pub extern "C" fn normalize_rule_ffi(line: *const c_char) -> *mut c_char {
    if line.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(line) };
    let r_str = String::from_utf8_lossy(c_str.to_bytes());
    
    match normalize_rule(&r_str) {
        Some(normalized) => {
            if let Ok(c_string) = std::ffi::CString::new(normalized) {
                c_string.into_raw()
            } else {
                std::ptr::null_mut()
            }
        }
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn strip_inline_comment_ffi(line: *const c_char) -> *mut c_char {
    if line.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(line) };
    let r_str = String::from_utf8_lossy(c_str.to_bytes());
    
    let stripped = strip_inline_comment(&r_str);
    if stripped.is_empty() {
        return std::ptr::null_mut();
    }
    
    if let Ok(c_string) = std::ffi::CString::new(stripped) {
        c_string.into_raw()
    } else {
        std::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn free_string_ffi(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = std::ffi::CString::from_raw(s);
    }
}

pub mod url_rewriter;

#[unsafe(no_mangle)]
pub extern "C" fn run_url_rewrites_ffi(directory: *const std::ffi::c_char) -> i32 {
    if directory.is_null() {
        return 0;
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(directory) };
    let r_str = String::from_utf8_lossy(c_str.to_bytes());
    url_rewriter::run_url_rewrites(&r_str)
}

#[unsafe(no_mangle)]
pub extern "C" fn copy_github_variants_ffi(root_dir: *const std::ffi::c_char) {
    if root_dir.is_null() {
        return;
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(root_dir) };
    let r_str = String::from_utf8_lossy(c_str.to_bytes());
    url_rewriter::copy_github_variants(&r_str)
}

#[unsafe(no_mangle)]
pub extern "C" fn safe_write_file_ffi(
    path: *const std::ffi::c_char,
    content: *const std::ffi::c_char,
    atomic: bool,
) -> bool {
    if path.is_null() || content.is_null() {
        return false;
    }
    let path_str = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
    let content_str = unsafe { std::ffi::CStr::from_ptr(content) }.to_string_lossy();

    let p = std::path::Path::new(path_str.as_ref());
    safe_write_file_internal(p, &content_str, atomic)
}

#[unsafe(no_mangle)]
pub extern "C" fn read_file_to_string_ffi(path: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    let path_str = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
    
    match std::fs::read_to_string(path_str.as_ref()) {
        Ok(content) => {
            if let Ok(c_string) = std::ffi::CString::new(content) {
                c_string.into_raw()
            } else {
                std::ptr::null_mut()
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn file_exists_ffi(path: *const std::ffi::c_char) -> bool {
    if path.is_null() {
        return false;
    }
    let path_str = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
    std::path::Path::new(path_str.as_ref()).exists()
}

#[unsafe(no_mangle)]
pub extern "C" fn ensure_dir_ffi(path: *const std::ffi::c_char) -> bool {
    if path.is_null() {
        return false;
    }
    let path_str = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
    std::fs::create_dir_all(path_str.as_ref()).is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn remove_file_ffi(path: *const std::ffi::c_char, missing_ok: bool) -> bool {
    if path.is_null() {
        return false;
    }
    let path_str = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
    let p = std::path::Path::new(path_str.as_ref());
    
    if !p.exists() {
        return missing_ok;
    }
    
    std::fs::remove_file(p).is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn remove_dir_all_ffi(path: *const std::ffi::c_char, missing_ok: bool) -> bool {
    if path.is_null() {
        return false;
    }
    let path_str = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
    let p = std::path::Path::new(path_str.as_ref());
    
    if !p.exists() {
        return missing_ok;
    }
    
    std::fs::remove_dir_all(p).is_ok()
}

pub fn safe_write_file_internal(p: &std::path::Path, content_str: &str, atomic: bool) -> bool {
    // Semantic Check
    if p.exists() {
        if let Ok(old_content) = std::fs::read_to_string(p) {
            let old_stripped = url_rewriter::get_semantic_content_pub(&old_content);
            let new_stripped = url_rewriter::get_semantic_content_pub(content_str);
            if old_stripped == new_stripped {
                println!("\x1b[0;34m[INFO]\x1b[0m Skipping write for {}: No semantic changes detected.", p.file_name().unwrap_or_default().to_string_lossy());
                return true;
            }
        }
    }

    if atomic {
        let parent = p.parent().unwrap_or(std::path::Path::new(""));
        let file_name = p.file_name().and_then(|f| f.to_str()).unwrap_or("file");
        let temp_path = parent.join(format!(".tmp_{}_{}.writing", file_name, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        match std::fs::write(&temp_path, content_str) {
            Ok(_) => {
                match std::fs::rename(&temp_path, p) {
                    Ok(_) => true,
                    Err(e) => {
                        eprintln!("\x1b[0;31m[ERROR]\x1b[0m Failed to rename temp file {:?} to {:?}: {:?}", temp_path, p, e);
                        let _ = std::fs::remove_file(&temp_path);
                        false
                    }
                }
            }
            Err(e) => {
                eprintln!("\x1b[0;31m[ERROR]\x1b[0m Failed to write temp file {:?}: {:?}", temp_path, e);
                false
            }
        }
    } else {
        match std::fs::write(p, content_str) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("\x1b[0;31m[ERROR]\x1b[0m Failed to write file {:?}: {:?}", p, e);
                false
            }
        }
    }
}

pub mod mitm_manager;

#[unsafe(no_mangle)]
pub extern "C" fn run_mitm_cleanup_ffi(directory: *const std::ffi::c_char, dry_run: bool) -> i32 {
    if directory.is_null() {
        return 0;
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(directory) };
    let r_str = String::from_utf8_lossy(c_str.to_bytes());
    mitm_manager::run_mitm_cleanup(&r_str, dry_run)
}

pub mod firewall_sync;

#[unsafe(no_mangle)]
pub extern "C" fn sync_ports_ffi(
    ports_source: *const std::ffi::c_char,
    firewall_modules: *const *const std::ffi::c_char,
    firewall_modules_len: i32,
    execute: bool,
    version: *const std::ffi::c_char,
) {
    if ports_source.is_null() || firewall_modules.is_null() || version.is_null() {
        return;
    }
    let source_str = unsafe { std::ffi::CStr::from_ptr(ports_source) }.to_string_lossy();
    let version_str = unsafe { std::ffi::CStr::from_ptr(version) }.to_string_lossy();

    let mut modules = Vec::new();
    for i in 0..firewall_modules_len {
        let ptr = unsafe { *firewall_modules.offset(i as isize) };
        if !ptr.is_null() {
            let s = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy();
            modules.push(s);
        }
    }

    let modules_refs: Vec<&str> = modules.iter().map(|s| s.as_ref()).collect();
    firewall_sync::sync_ports(&source_str, &modules_refs, execute, &version_str);
}

pub mod smart_cleanup;

#[unsafe(no_mangle)]
pub extern "C" fn run_cleanup_ffi(root_dir: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if root_dir.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(root_dir) };
    let r_str = String::from_utf8_lossy(c_str.to_bytes());
    
    let stats = smart_cleanup::run_cleanup(&r_str);
    
    if let Ok(json_str) = serde_json::to_string(&stats) {
        if let Ok(c_string) = std::ffi::CString::new(json_str) {
            return c_string.into_raw();
        }
    }
    std::ptr::null_mut()
}

pub mod promax;
pub mod merge_bundles;

#[unsafe(no_mangle)]
pub extern "C" fn run_adblock_manager_ffi(
    root_dir: *const std::ffi::c_char,
    execute: bool,
) -> bool {
    if root_dir.is_null() {
        return false;
    }
    let root_str = unsafe { std::ffi::CStr::from_ptr(root_dir) }.to_string_lossy();
    promax::run_adblock_manager(&root_str, execute)
}

#[unsafe(no_mangle)]
pub extern "C" fn process_merge_bundle_ffi(json: *const std::ffi::c_char) -> bool {
    if json.is_null() {
        return false;
    }
    let json_str = unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy();
    merge_bundles::run_merge_bundle_json(&json_str)
}

pub mod srs_generator;

#[unsafe(no_mangle)]
pub extern "C" fn run_srs_generator_ffi(
    root_dir: *const std::ffi::c_char,
    singbox_path: *const std::ffi::c_char,
) -> bool {
    if root_dir.is_null() || singbox_path.is_null() {
        return false;
    }
    let root_str = unsafe { std::ffi::CStr::from_ptr(root_dir) }.to_string_lossy();
    let singbox_str = unsafe { std::ffi::CStr::from_ptr(singbox_path) }.to_string_lossy();
    srs_generator::run_srs_generator(&root_str, &singbox_str)
}
