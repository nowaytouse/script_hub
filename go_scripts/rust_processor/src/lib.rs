use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Write};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::Path;
use regex::Regex;
use once_cell::sync::Lazy;

static DOMAIN_RULE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^DOMAIN,([^,]+)$").unwrap());
static DOMAIN_SUFFIX_RULE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^DOMAIN-SUFFIX,([^,]+)$").unwrap());
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

fn strip_inline_comment(line: &str) -> String {
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
    
    // Read local files directly in Rust
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

    // Process remote content downloaded by Go
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
        if let Ok(mut file) = File::create(&target_file_path) {
            let header_str = generate_header(&name_str, &policy_str, &node_str, &desc_str, non_ip_rules.len());
            let _ = write!(file, "{}", header_str);
            for r in &non_ip_rules {
                let _ = writeln!(file, "{}", r);
            }
        } else {
            return false;
        }
    } else if target_file_path.exists() {
        let _ = fs::remove_file(&target_file_path);
    }
    
    if !ip_rules.is_empty() {
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
    } else if target_ip_file_path.exists() {
        let _ = fs::remove_file(&target_ip_file_path);
    }

    true
}
