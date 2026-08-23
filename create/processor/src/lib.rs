use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub fn publication_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(
        &chrono::FixedOffset::east_opt(8 * 60 * 60).expect("UTC+8 is a valid fixed offset"),
    )
}

static VALID_IPV4_CIDR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d{1,3}\.){3}\d{1,3}(/\d{1,2})?$").unwrap());
static VALID_IPV6_CIDR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([0-9a-fA-F:]+)(/\d{1,3})?$").unwrap());

static M1: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\(\^\|\.\)(.+?)-\.\+\.").unwrap());
static M2: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:\(\^\|\.\)|\^)([a-zA-Z0-9][-a-zA-Z0-9.]*)$").unwrap());
static M3: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\.\+(.+)$").unwrap());

fn is_invalid_domain_regex(payload: &str) -> bool {
    let val = payload.trim();
    val.len() < 2 || ["$", ",", "-", ".", "2", "6", "]", "["].contains(&val)
}

fn is_invalid_url_regex(payload: &str) -> bool {
    let val = payload.trim().to_lowercase();
    val.len() < 3
        || [
            "https:",
            "http:",
            "https",
            "http",
            "^https?:",
            "^https?://",
            "^https?:\\/\\/",
        ]
        .contains(&val.as_str())
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

fn validate_rule_value(rule_type: &str, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    match rule_type {
        "DOMAIN" | "DOMAIN-SUFFIX" => {
            let val = value.trim_start_matches('.');
            val.parse::<IpAddr>().is_err() && (val.contains('.') || val.len() > 2)
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
        line.split(',').next().unwrap().to_uppercase()
    };

    let payload_safe = ["DOMAIN-REGEX", "URL-REGEX", "USER-AGENT", "PROCESS-NAME"];
    if payload_safe.contains(&head.as_str()) {
        return line.to_string();
    }

    let comment_start = [" //", "\t//", " #", "\t#"]
        .iter()
        .filter_map(|marker| line.find(marker))
        .min();
    let cleaned = comment_start.map_or(line, |index| &line[..index]);
    cleaned.trim().to_string()
}

pub fn is_ruleset_watermark(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.ends_with(".skk.moe")
        && (value.contains("ru1353t.1s.m4d3.by.5ukk4w")
            || value.contains("ru1353t_1s_m4d3_by_5ukk4w")
            || value.contains("rul35et_i5_mad3_by_5ukk4w")
            || value.contains("rule5et_1s_m4d3_by_5ukk4w")
            || value.contains("ruleset_is_made_by_sukkaw"))
}

fn normalize_rule(line: &str) -> Option<String> {
    let line = strip_inline_comment(line);
    if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
        return None;
    }
    let rule = crate::promax::rule::SurgeRule::parse(&line).ok()?;
    let rule_type = rule.kind.as_str();

    let valid_types = [
        "DOMAIN",
        "DOMAIN-SUFFIX",
        "DOMAIN-KEYWORD",
        "IP-CIDR",
        "IP-CIDR6",
        "DOMAIN-REGEX",
        "GEOIP",
        "USER-AGENT",
        "URL-REGEX",
        "PROCESS-NAME",
    ];
    if !valid_types.contains(&rule_type) {
        return None;
    }

    if is_ruleset_watermark(&rule.payload) {
        return None;
    }

    if !validate_rule_value(rule_type, &rule.payload) {
        return None;
    }

    Some(rule.render_external())
}

fn clean_rule(
    rule: &str,
    ruleset_name: &str,
    conflict_domains: &[String],
    skip_conflict: bool,
) -> Option<String> {
    let mut r = rule.trim().to_string();
    if r.is_empty() || r.starts_with('#') {
        return None;
    }
    if r == "payload:" || r == "payload" {
        return None;
    }
    if r.starts_with('-') {
        r = r[1..]
            .trim()
            .trim_matches(|c| c == '\'' || c == '"')
            .to_string();
        if r.is_empty() {
            return None;
        }
    }

    if !r.contains(',') {
        if r.contains(':')
            || (r.split('.').count() == 4
                && r.split('.')
                    .next()
                    .unwrap()
                    .chars()
                    .all(|c| c.is_ascii_digit()))
        {
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
            "google",
            "gstatic",
            "gmail",
            "ggpht",
            "youtube",
            "ytimg",
            "facebook",
            "fbcdn",
            "instagram",
            "twitter",
            "twimg",
            "t.co",
            "telegram",
            "netflix",
            "nflxvideo",
            "nflxext",
            "disney",
            "github",
            "akamai",
            "fastly",
            "cloudflare",
            "browserleaks",
            "byteoversea",
            "tiktok",
            "tiktokv",
            "tiktokcdn",
            "tiktokeu",
            "tiktokw",
            "muscdn",
            "tiktokrow",
            "lark.com",
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
        old_rules.sort();
        let mut canonical_new = new_rules.to_vec();
        canonical_new.sort();
        return old_rules == canonical_new;
    }
    false
}

pub struct RulesetInput<'a> {
    pub name: &'a str,
    pub target_dir: &'a Path,
    pub conflict_domains: &'a [String],
    pub skip_conflict: bool,
    pub policy: &'a str,
    pub node: &'a str,
    pub description: &'a str,
    pub local_paths: &'a [PathBuf],
    pub remote_content: &'a str,
}

pub fn process_ruleset(input: RulesetInput<'_>) -> Result<(), String> {
    let RulesetInput {
        name,
        target_dir,
        conflict_domains,
        skip_conflict,
        policy,
        node,
        description,
        local_paths,
        remote_content,
    } = input;

    let mut all_rules = HashSet::new();

    for path in local_paths {
        let content = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        for line in content.lines() {
            if let Some(cleaned) = clean_rule(line, name, conflict_domains, skip_conflict) {
                all_rules.insert(cleaned);
            }
        }
    }

    for line in remote_content.lines() {
        if let Some(cleaned) = clean_rule(line, name, conflict_domains, skip_conflict) {
            all_rules.insert(cleaned);
        }
    }

    let mut ip_rules = Vec::new();
    let mut non_ip_rules = Vec::new();

    for rule in all_rules {
        if name != "Direct"
            && (rule.starts_with("IP-CIDR,")
                || rule.starts_with("IP-CIDR6,")
                || rule.starts_with("IP-ASN,")
                || rule.starts_with("GEOIP,"))
        {
            ip_rules.push(rule);
        } else {
            non_ip_rules.push(rule);
        }
    }

    ip_rules.sort();
    non_ip_rules.sort();

    let target_file_path = target_dir.join(format!("{name}.list"));
    let target_ip_file_path = target_dir.join(format!("{name}_ip.list"));

    if !non_ip_rules.is_empty() {
        if semantic_rules_match(&target_file_path, &non_ip_rules) {
            println!(
                "\x1b[0;34m[INFO]\x1b[0m Skipping write for {}: No semantic changes detected.",
                target_file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
        } else {
            let mut file = File::create(&target_file_path).map_err(|error| {
                format!("failed to create {}: {error}", target_file_path.display())
            })?;
            let header_str = generate_header(name, policy, node, description, non_ip_rules.len());
            write!(file, "{header_str}").map_err(|error| error.to_string())?;
            for rule in &non_ip_rules {
                writeln!(file, "{rule}").map_err(|error| error.to_string())?;
            }
        }
    } else if target_file_path.exists() {
        let _ = fs::remove_file(&target_file_path);
    }

    if !ip_rules.is_empty() {
        if semantic_rules_match(&target_ip_file_path, &ip_rules) {
            println!(
                "\x1b[0;34m[INFO]\x1b[0m Skipping write for {}: No semantic changes detected.",
                target_ip_file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
        } else {
            let mut file = File::create(&target_ip_file_path).map_err(|error| {
                format!(
                    "failed to create {}: {error}",
                    target_ip_file_path.display()
                )
            })?;
            let ip_name = format!("{name}_ip");
            let header_str = generate_header(&ip_name, policy, node, description, ip_rules.len());
            write!(file, "{header_str}").map_err(|error| error.to_string())?;
            for rule in &ip_rules {
                writeln!(file, "{rule}").map_err(|error| error.to_string())?;
            }
        }
    } else if target_ip_file_path.exists() {
        let _ = fs::remove_file(&target_ip_file_path);
    }

    Ok(())
}

pub mod url_rewriter;

pub fn safe_write_file_internal(p: &std::path::Path, content_str: &str, atomic: bool) -> bool {
    // Semantic Check
    if p.exists()
        && let Ok(old_content) = std::fs::read_to_string(p)
    {
        let old_stripped = url_rewriter::semantic_content_for_path_pub(p, &old_content);
        let new_stripped = url_rewriter::semantic_content_for_path_pub(p, content_str);
        if old_stripped == new_stripped {
            println!(
                "\x1b[0;34m[INFO]\x1b[0m Skipping write for {}: No semantic changes detected.",
                p.file_name().unwrap_or_default().to_string_lossy()
            );
            return true;
        }
    }

    if atomic {
        let parent = p.parent().unwrap_or(std::path::Path::new(""));
        let file_name = p.file_name().and_then(|f| f.to_str()).unwrap_or("file");
        let temp_path = parent.join(format!(
            ".tmp_{}_{}.writing",
            file_name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        match std::fs::write(&temp_path, content_str) {
            Ok(_) => match std::fs::rename(&temp_path, p) {
                Ok(_) => true,
                Err(e) => {
                    eprintln!(
                        "\x1b[0;31m[ERROR]\x1b[0m Failed to rename temp file {:?} to {:?}: {:?}",
                        temp_path, p, e
                    );
                    let _ = std::fs::remove_file(&temp_path);
                    false
                }
            },
            Err(e) => {
                eprintln!(
                    "\x1b[0;31m[ERROR]\x1b[0m Failed to write temp file {:?}: {:?}",
                    temp_path, e
                );
                false
            }
        }
    } else {
        match std::fs::write(p, content_str) {
            Ok(_) => true,
            Err(e) => {
                eprintln!(
                    "\x1b[0;31m[ERROR]\x1b[0m Failed to write file {:?}: {:?}",
                    p, e
                );
                false
            }
        }
    }
}

pub mod mitm_manager;

pub mod firewall_sync;

pub mod smart_cleanup;

pub mod functional_update;
pub mod general_update;
pub mod merge_bundles;
pub mod module_catalog;
pub mod promax;

pub mod srs_generator;

#[cfg(test)]
mod tests {
    use super::{is_ruleset_watermark, normalize_rule, strip_inline_comment};

    #[test]
    fn inline_comment_stripping_preserves_urls() {
        assert_eq!(
            strip_inline_comment(
                "RULE-SET,https://example.test/rules.list,REJECT # refreshed upstream"
            ),
            "RULE-SET,https://example.test/rules.list,REJECT"
        );
        assert_eq!(
            strip_inline_comment("DEST-PORT,3306,REJECT // database"),
            "DEST-PORT,3306,REJECT"
        );
    }

    #[test]
    fn identifies_upstream_ruleset_watermarks_without_matching_normal_hosts() {
        assert!(is_ruleset_watermark(
            "7h15.ru1353t.1s.m4d3.by.5ukk4w.skk.moe"
        ));
        assert!(is_ruleset_watermark(
            "this_ruleset_is_made_by_sukkaw.ruleset.skk.moe"
        ));
        assert!(!is_ruleset_watermark("cdn.skk.moe"));
    }

    #[test]
    fn domain_rules_reject_ip_literals_instead_of_retyping_them() {
        assert_eq!(normalize_rule("DOMAIN,10.10.34.34"), None);
        assert_eq!(normalize_rule("DOMAIN-SUFFIX,180.76.76.200"), None);
        assert_eq!(
            normalize_rule("IP-CIDR,180.76.76.200/32"),
            Some("IP-CIDR,180.76.76.200/32".to_string())
        );
    }

    #[test]
    fn external_rules_drop_inline_policy_but_keep_supported_options() {
        assert_eq!(
            normalize_rule("DOMAIN,dns.alidns.com,DIRECT"),
            Some("DOMAIN,dns.alidns.com".to_string())
        );
        assert_eq!(
            normalize_rule("IP-CIDR,223.5.5.5/32,DIRECT,no-resolve"),
            Some("IP-CIDR,223.5.5.5/32,NO-RESOLVE".to_string())
        );
    }
}
