use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub(crate) fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let host = authority.rsplit('@').next().unwrap_or(authority);
    format!("{scheme}://{host}{}", &rest[authority_end..])
}

pub fn publication_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(
        &chrono::FixedOffset::east_opt(8 * 60 * 60).expect("UTC+8 is a valid fixed offset"),
    )
}

pub(crate) fn verify_missing_directory(directory: &Path) -> Result<(), String> {
    // Windows may report NotFound when an ancestor is a file, not a missing directory.
    for ancestor in directory.ancestors() {
        match fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() && ancestor != directory => return Ok(()),
            Ok(_) => {
                return Err(format!(
                    "cannot verify missing directory {} at {}",
                    directory.display(),
                    ancestor.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::symlink_metadata(ancestor) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(format!("cannot inspect {}: {error}", ancestor.display()));
                    }
                    Ok(_) => {
                        return Err(format!("unresolved directory path: {}", ancestor.display()));
                    }
                }
            }
            Err(error) => return Err(format!("cannot inspect {}: {error}", ancestor.display())),
        }
    }
    Err(format!("cannot verify a parent of {}", directory.display()))
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
        "DOMAIN-WILDCARD",
        "IP-CIDR",
        "IP-CIDR6",
        "IP-ASN",
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
    _ruleset_name: &str,
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
        let payload = normalized.split(',').nth(1);
        for dom in conflict_domains {
            if payload == Some(dom.as_str()) {
                return None;
            }
        }
    }

    Some(normalized)
}

fn generate_header(name: &str, desc: &str, count: usize) -> String {
    let mut header = vec![
        format!("# Ruleset: {}", name),
        "# Policy: USER-DEFINED".to_string(),
    ];
    header.push(format!("# Description: {}", desc));
    header.push(format!("# Rules: {}", count));

    header.join("\n") + "\n"
}

fn semantic_rules_match(path: &Path, sorted_new_rules: &[String]) -> bool {
    if let Ok(content) = fs::read_to_string(path) {
        if !content
            .lines()
            .any(|line| line.trim() == "# Policy: USER-DEFINED")
            || content
                .lines()
                .any(|line| line.trim_start().starts_with("# Node:"))
        {
            return false;
        }
        let expected_count = format!("# Rules: {}", sorted_new_rules.len());
        let mut rule_count_headers = content
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("# Rules:"));
        if rule_count_headers.next() != Some(expected_count.as_str())
            || rule_count_headers.next().is_some()
        {
            return false;
        }
        let mut old_rules = Vec::new();
        for line in content.lines() {
            let stripped = line.trim();
            if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with("//") {
                continue;
            }
            old_rules.push(stripped.to_string());
        }
        old_rules.sort();
        return old_rules.as_slice() == sorted_new_rules;
    }
    false
}

pub struct RulesetInput<'a> {
    pub name: &'a str,
    pub target_dir: &'a Path,
    pub conflict_domains: &'a [String],
    pub skip_conflict: bool,
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

    if all_rules.is_empty() {
        return Err(format!(
            "ruleset {name} contains no valid rules; preserving existing outputs"
        ));
    }

    let mut domain_rules = Vec::new();
    let mut ip_rules = Vec::new();
    let mut extra_rules = Vec::new();

    for rule in all_rules {
        let rule_type = rule.split(',').next().unwrap_or_default();
        if matches!(
            rule_type,
            "DOMAIN" | "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" | "DOMAIN-WILDCARD" | "DOMAIN-REGEX"
        ) {
            domain_rules.push(rule);
        } else if matches!(rule_type, "IP-CIDR" | "IP-CIDR6" | "IP-ASN" | "GEOIP") {
            ip_rules.push(rule);
        } else {
            extra_rules.push(rule);
        }
    }

    domain_rules.sort();
    ip_rules.sort();
    extra_rules.sort();

    let target_file_path = target_dir.join(format!("{name}.list"));
    let target_ip_file_path = target_dir.join(format!("{name}_ip.list"));
    let target_extra_file_path = target_dir.join(format!("{name}_extra.list"));

    for (path, output_name, rules) in [
        (&target_file_path, name.to_string(), &domain_rules),
        (&target_ip_file_path, format!("{name}_ip"), &ip_rules),
        (
            &target_extra_file_path,
            format!("{name}_extra"),
            &extra_rules,
        ),
    ] {
        if rules.is_empty() {
            match fs::remove_file(path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => {
                    return Err(format!(
                        "failed to remove empty ruleset {}: {error}",
                        path.display()
                    ));
                }
            }
            continue;
        }
        if semantic_rules_match(path, rules) {
            continue;
        }
        let content = format!(
            "{}{}",
            generate_header(&output_name, description, rules.len()),
            if rules.is_empty() {
                String::new()
            } else {
                rules.join("\n") + "\n"
            }
        );
        if !safe_write_file_internal(path, &content, true) {
            return Err(format!("failed to atomically publish {}", path.display()));
        }
    }

    Ok(())
}

fn normalize_generated_text(content: &str) -> String {
    let mut lines = content.lines().map(str::trim_end).collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    }
}

pub fn safe_write_file_internal(p: &std::path::Path, content_str: &str, atomic: bool) -> bool {
    let mut normalized_content = normalize_generated_text(content_str);

    // Semantic Check
    if p.exists()
        && let Ok(old_content) = std::fs::read_to_string(p)
    {
        let old_stripped = url_rewriter::semantic_content_for_path_pub(p, &old_content);
        let new_stripped = url_rewriter::semantic_content_for_path_pub(p, &normalized_content);
        if old_stripped == new_stripped {
            let normalized_old = normalize_generated_text(&old_content);
            if normalized_old == old_content {
                println!(
                    "\x1b[0;34m[INFO]\x1b[0m Skipping write for {}: No semantic changes detected.",
                    p.file_name().unwrap_or_default().to_string_lossy()
                );
                return true;
            }
            normalized_content = normalized_old;
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
        match std::fs::write(&temp_path, &normalized_content) {
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
        match std::fs::write(p, &normalized_content) {
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

pub mod promax;

pub mod app;
pub mod desktop;
pub mod pipeline;
pub mod transforms;

// Preserve the historical crate-root module paths while the implementation is
// organized under its owning subsystems.
pub use app::{update_cli, validate_cli};
pub use desktop::job as gui_job;
pub use pipeline::{
    firewall_sync, functional_update, general_update, merge_bundles, module_catalog, srs_generator,
};
pub use transforms::{mitm_manager, smart_cleanup, url_rewriter};

#[cfg(test)]
mod tests {
    use super::{
        RulesetInput, is_ruleset_watermark, normalize_generated_text, normalize_rule,
        process_ruleset, strip_inline_comment,
    };
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn url_diagnostics_remove_credentials_queries_and_fragments() {
        assert_eq!(
            super::redact_url("https://user:secret@example.test/rules.list?token=private#hidden"),
            "https://example.test/rules.list"
        );
        assert_eq!(
            super::redact_url("https://user:secret@example.test?token=private"),
            "https://example.test"
        );
        assert_eq!(
            super::redact_url("https://cdn.jsdelivr.net/gh/owner/repo@master/list"),
            "https://cdn.jsdelivr.net/gh/owner/repo@master/list"
        );
    }

    #[test]
    fn missing_directory_check_distinguishes_absence_from_blocking_file() {
        let root = output_dir();
        let missing = root.join("missing/child");
        assert!(super::verify_missing_directory(&missing).is_ok());
        fs::write(root.join("missing"), "not a directory").unwrap();
        assert!(super::verify_missing_directory(&missing).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    fn output_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "script-hub-ruleset-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn process(
        name: &str,
        target_dir: &std::path::Path,
        remote_content: &str,
    ) -> Result<(), String> {
        process_ruleset(RulesetInput {
            name,
            target_dir,
            conflict_domains: &[],
            skip_conflict: true,
            description: "test",
            local_paths: &[],
            remote_content,
        })
    }

    #[test]
    fn all_invalid_input_preserves_existing_outputs() {
        let dir = output_dir();
        let domain = dir.join("test.list");
        let ip = dir.join("test_ip.list");
        let extra = dir.join("test_extra.list");
        fs::write(
            &domain,
            "# Ruleset: test\n# Policy: USER-DEFINED\n# Rules: 1\nDOMAIN,old.example\n",
        )
        .unwrap();
        fs::write(
            &ip,
            "# Ruleset: test_ip\n# Policy: USER-DEFINED\n# Rules: 1\nIP-CIDR,10.0.0.0/8\n",
        )
        .unwrap();
        fs::write(&extra, "# Ruleset: test_extra\n# Policy: USER-DEFINED\n# Rules: 1\nURL-REGEX,^https://old\\.example\n").unwrap();

        assert!(process("test", &dir, "DOMAIN,").is_err());
        assert!(
            fs::read_to_string(domain)
                .unwrap()
                .contains("DOMAIN,old.example\n")
        );
        assert!(
            fs::read_to_string(ip)
                .unwrap()
                .contains("IP-CIDR,10.0.0.0/8\n")
        );
        assert!(
            fs::read_to_string(extra)
                .unwrap()
                .contains("URL-REGEX,^https://old\\.example\n")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn empty_ip_family_is_removed_without_writing_an_empty_file() {
        let dir = output_dir();
        let ip = dir.join("test_ip.list");
        fs::write(&ip, "IP-CIDR,10.0.0.0/8\n").unwrap();

        process("test", &dir, "DOMAIN,example.test").unwrap();
        assert!(!ip.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_empty_family_removal_is_returned() {
        let dir = output_dir();
        fs::create_dir(dir.join("test_ip.list")).unwrap();

        let error = process("test", &dir, "DOMAIN,example.test").unwrap_err();
        assert!(error.contains("failed to remove empty ruleset"));
        assert!(dir.join("test_ip.list").is_dir());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn direct_and_mixed_rulesets_split_domain_ip_and_extra_rules() {
        let dir = output_dir();
        let input = "DOMAIN,example.test\nDOMAIN-WILDCARD,api-*.example.test\nIP-CIDR,10.0.0.0/8\nIP-ASN,123\nGEOIP,CN\nURL-REGEX,^https://example\\.test\n";
        for name in ["Direct", "mixed"] {
            process(name, &dir, input).unwrap();
            let domain = fs::read_to_string(dir.join(format!("{name}.list"))).unwrap();
            let ip = fs::read_to_string(dir.join(format!("{name}_ip.list"))).unwrap();
            let extra = fs::read_to_string(dir.join(format!("{name}_extra.list"))).unwrap();
            assert!(domain.contains("DOMAIN,example.test\n"));
            assert!(domain.contains("DOMAIN-WILDCARD,api-*.example.test\n"));
            assert!(domain.contains("# Rules: 2\n"));
            assert!(!domain.contains("IP-CIDR"));
            assert!(ip.contains("IP-CIDR,10.0.0.0/8\n"));
            assert!(ip.contains("IP-ASN,123\n"));
            assert!(ip.contains("GEOIP,CN\n"));
            assert!(ip.contains("# Rules: 3\n"));
            assert!(extra.contains("URL-REGEX,^https://example\\.test\n"));
            assert!(extra.contains("# Rules: 1\n"));
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn empty_groups_are_not_published_and_stale_extra_is_removed() {
        let dir = output_dir();
        let extra = dir.join("test_extra.list");
        fs::write(&extra, "# old extra\n").unwrap();

        process("test", &dir, "IP-CIDR,10.0.0.0/8").unwrap();
        assert!(!dir.join("test.list").exists());
        assert!(
            fs::read_to_string(dir.join("test_ip.list"))
                .unwrap()
                .contains("IP-CIDR,10.0.0.0/8\n")
        );
        assert!(!extra.exists());
        process("test", &dir, "IP-CIDR,10.0.0.0/8").unwrap();
        assert!(!dir.join("test.list").exists());
        assert!(!extra.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn generated_ruleset_metadata_never_recommends_routing() {
        for name in ["NSFW", "Apple", "Direct", "GlobalProxy"] {
            let header = super::generate_header(name, "classification only", 1);
            assert!(header.lines().any(|line| line == "# Policy: USER-DEFINED"));
            for route in ["DIRECT", "PROXY", "REJECT"] {
                assert!(!header.contains(&format!("# Policy: {route}")));
            }
            assert!(!header.contains("# Node:"));
        }
    }

    #[test]
    fn exported_classification_rules_have_no_inline_routing_policy() {
        let dir = output_dir();
        process("NSFW", &dir, "DOMAIN,example.test,PROXY").unwrap();
        let output = fs::read_to_string(dir.join("NSFW.list")).unwrap();
        assert!(output.contains("# Policy: USER-DEFINED\n"));
        assert!(output.contains("DOMAIN,example.test\n"));
        assert!(!output.contains("# Policy: PROXY"));
        assert!(!output.contains("DOMAIN,example.test,PROXY"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn neutral_metadata_migrates_once_without_reordering_unchanged_rules() {
        let dir = output_dir();
        let path = dir.join("test.list");
        fs::write(
            &path,
            "# Ruleset: test\n# Policy: PROXY\n# Node: Any\nDOMAIN,a.example\nDOMAIN,b.example\n",
        )
        .unwrap();
        process("test", &dir, "DOMAIN,b.example\nDOMAIN,a.example").unwrap();
        let neutral = fs::read_to_string(&path).unwrap();
        assert!(neutral.contains("# Policy: USER-DEFINED\n"));
        assert!(!neutral.contains("# Node:"));
        process(
            "test",
            &dir,
            "DOMAIN,a.example\nDOMAIN,b.example\n# different input order/comment",
        )
        .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), neutral);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn incorrect_rules_header_count_is_repaired_to_the_actual_count() {
        let dir = output_dir();
        let path = dir.join("test.list");
        fs::write(
            &path,
            "# Ruleset: test\n# Policy: USER-DEFINED\n# Rules: 999999\nDOMAIN,a.example\nDOMAIN,b.example\n",
        )
        .unwrap();

        process("test", &dir, "DOMAIN,a.example\nDOMAIN,b.example").unwrap();
        let output = fs::read_to_string(&path).unwrap();
        assert!(output.contains("# Rules: 2\n"));
        assert!(!output.contains("# Rules: 999999\n"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn correct_rules_count_does_not_rewrite_other_header_formatting() {
        let dir = output_dir();
        let path = dir.join("test.list");
        let existing = "# Ruleset: test\n# Policy: USER-DEFINED\n# Updated: 2026-01-02\n# Rules: 2\nDOMAIN,a.example\nDOMAIN,b.example\n";
        fs::write(&path, existing).unwrap();

        process("test", &dir, "DOMAIN,a.example\nDOMAIN,b.example").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), existing);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn category_names_do_not_impose_hidden_proxy_decisions() {
        for domain in ["project.com", "google.example", "github.example"] {
            let line = format!("DOMAIN,{domain}");
            assert_eq!(super::clean_rule(&line, "Direct", &[], false), Some(line));
        }
    }

    #[test]
    fn publication_failure_is_returned() {
        let dir = output_dir();
        fs::create_dir(dir.join("test.list")).unwrap();
        assert!(
            process("test", &dir, "DOMAIN,example.test")
                .unwrap_err()
                .contains("failed to atomically publish")
        );
        assert!(dir.join("test.list").is_dir());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn generated_text_has_clean_line_endings_and_one_final_newline() {
        assert_eq!(
            normalize_generated_text("# comment \r\nvalue\t\r\n\r\n"),
            "# comment\nvalue\n"
        );
        assert_eq!(normalize_generated_text(""), "");
    }

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
    fn conflict_domains_match_the_payload_even_with_rule_options() {
        let conflicts = vec!["sentry.io".to_string()];
        assert_eq!(
            super::clean_rule(
                "DOMAIN-SUFFIX,sentry.io,extended-matching",
                "AI",
                &conflicts,
                false
            ),
            None,
        );
        assert_eq!(
            super::clean_rule(
                "DOMAIN,o33249.ingest.sentry.io,extended-matching",
                "AI",
                &conflicts,
                false
            ),
            Some("DOMAIN,o33249.ingest.sentry.io,extended-matching".to_string()),
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
            Some("IP-CIDR,223.5.5.5/32,no-resolve".to_string())
        );
    }
}
