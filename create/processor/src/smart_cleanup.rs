use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use regex::Regex;
use once_cell::sync::Lazy;

static PRIORITY_ORDER: &[&str] = &[
    "AdBlock_",
    "AdBlock.list",
    "reject-drop.list",
    "reject-no-drop.list",
    "HTTPDNS_Hijack.list",
    "AI.list",
    "AI_ip.list",
    "SocialMedia.list",
    "SocialMedia_ip.list",
    "NSFW.list",
    "NSFW_ip.list",
    "substore.list",
    "substore_ip.list",
    "AppleNews.list",
    "AppleNews_ip.list",
    "StreamUS.list",
    "StreamUS_ip.list",
    "StreamJP.list",
    "StreamJP_ip.list",
    "StreamKR.list",
    "StreamKR_ip.list",
    "StreamEU.list",
    "StreamEU_ip.list",
    "StreamHK.list",
    "StreamHK_ip.list",
    "StreamTW.list",
    "StreamTW_ip.list",
    "Spotify.list",
    "Spotify_ip.list",
    "Gaming.list",
    "Gaming_ip.list",
    "Google.list",
    "Google_ip.list",
    "Apple.list",
    "Apple_ip.list",
    "Microsoft.list",
    "Microsoft_ip.list",
    "GitHub.list",
    "GitHub_ip.list",
    "PayPal.list",
    "PayPal_ip.list",
    "Binance.list",
    "Binance_ip.list",
    "Cloudflare.list",
    "Cloudflare_ip.list",
    "Bilibili.list",
    "Bilibili_ip.list",
    "CDN.list",
    "CDN_ip.list",
    "Direct.list",
    "Direct_ip.list",
    "GlobalProxy.list",
    "GlobalProxy_ip.list",
];

static MANDATORY_PROXY_KEYWORDS: &[&str] = &[
    "google", "gstatic", "gmail", "ggpht", "youtube", "ytimg",
    "facebook", "fbcdn", "instagram", "twitter", "twimg", "t.co",
    "telegram", "netflix", "nflxvideo", "nflxext", "disney", "github",
    "akamai", "fastly", "cloudflare", "browserleaks",
    "byteoversea", "tiktok", "tiktokv", "tiktokcdn", "tiktokeu",
    "tiktokw", "muscdn", "tiktokrow", "lark.com",
];

static ADBLOCK_ONLY_PREFIXES: &[&str] = &[
    "AdBlock_", "AdBlock.list",
];

static NSFW_CONTAMINATION_KEYWORDS: &[&str] = &[
    "porn", "hentai", "nsfw", "xxx", "adult", "sex", "erotic",
    "nude", "naked", "fetish", "bdsm", "onlyfans",
    "facebookporn", "facebookofsex", "facboox", "faceboox", "faseboox", "feceboox",
];

static SEMANTIC_COVERAGE_RULESETS: &[&str] = &[
    "AdBlock_",
    "AdBlock.list",
    "AI.list",
    "SocialMedia.list",
    "NSFW.list",
    "StreamUS.list",
    "StreamJP.list",
    "StreamKR.list",
    "StreamEU.list",
    "StreamHK.list",
    "StreamTW.list",
    "Spotify.list",
    "Gaming.list",
    "Google.list",
    "Apple.list",
    "AppleNews.list",
    "Microsoft.list",
    "GitHub.list",
    "PayPal.list",
    "Binance.list",
    "Cloudflare.list",
    "Bilibili.list",
];

static HEADER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^#\s*(Rules|Total|Total Rules):\s*").unwrap()
});

fn get_rank(rule_type: &str) -> i32 {
    match rule_type {
        "DOMAIN" => 0,
        "DOMAIN-SUFFIX" => 1,
        "DOMAIN-KEYWORD" => 2,
        "DOMAIN-WILDCARD" => 3,
        "DOMAIN-REGEX" => 4,
        "IP-CIDR" | "IP-CIDR6" => 5,
        "GEOIP" => 6,
        "PROCESS-NAME" => 7,
        "USER-AGENT" => 8,
        "URL-REGEX" => 9,
        _ => 999,
    }
}

fn is_semantic_type(rule_type: &str) -> bool {
    rule_type == "DOMAIN" || rule_type == "DOMAIN-SUFFIX"
}

fn is_valid_rule(line: &str) -> bool {
    if line.starts_with("RULE-SET") {
        return false;
    }
    if line.starts_with("DOMAIN") && line.contains(",no-resolve") {
        return false;
    }
    line.contains(',')
}

fn clean_rule(line: &str) -> String {
    let stripped = crate::strip_inline_comment(line);
    if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with("//") {
        return String::new();
    }
    if stripped.starts_with("DOMAIN") && stripped.contains(",no-resolve") {
        stripped.replace(",no-resolve", "")
    } else {
        stripped
    }
}

fn extract_payload(rule: &str) -> String {
    if !rule.contains(',') {
        return String::new();
    }
    let parts: Vec<&str> = rule.splitn(3, ',').collect();
    if parts.len() >= 2 {
        parts[1].trim().to_lowercase()
    } else {
        String::new()
    }
}

fn extract_type(rule: &str) -> String {
    let parts: Vec<&str> = rule.splitn(2, ',').collect();
    parts[0].trim().to_uppercase()
}

fn prefer_rule(current: &str, candidate: &str) -> String {
    let current_rank = get_rank(&extract_type(current));
    let candidate_rank = get_rank(&extract_type(candidate));
    if candidate_rank < current_rank {
        candidate.to_string()
    } else if candidate_rank == current_rank && candidate < current {
        candidate.to_string()
    } else {
        current.to_string()
    }
}

fn is_proxyish_domain(rule: &str) -> bool {
    let payload = extract_payload(rule);
    if payload.is_empty() {
        return false;
    }
    let rtype = extract_type(rule);
    let valid_types = ["DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "DOMAIN-WILDCARD", "DOMAIN-REGEX"];
    if !valid_types.contains(&rtype.as_str()) {
        return false;
    }
    for kw in MANDATORY_PROXY_KEYWORDS {
        if payload.contains(kw) {
            return true;
        }
    }
    false
}

fn is_cn_tld(rule: &str) -> bool {
    let payload = extract_payload(rule);
    if payload.is_empty() {
        return false;
    }
    let rtype = extract_type(rule);
    if rtype != "DOMAIN" && rtype != "DOMAIN-SUFFIX" {
        return false;
    }
    payload == "cn" || payload.ends_with(".cn")
}

fn iter_domain_parents(payload: &str) -> Vec<String> {
    let parts: Vec<&str> = payload.split('.').collect();
    if parts.len() < 2 {
        return vec![payload.to_string()];
    }
    let mut res = Vec::new();
    for i in 0..(parts.len() - 1) {
        res.push(parts[i..].join("."));
    }
    res
}

#[derive(Eq, PartialEq)]
struct SortedRuleItem {
    payload: String,
    rule: String,
    sort_key_0: i32,
    sort_key_1: usize,
    sort_key_2: usize,
    sort_key_3: i32,
}

impl Ord for SortedRuleItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_key_0.cmp(&other.sort_key_0)
            .then_with(|| self.sort_key_1.cmp(&other.sort_key_1))
            .then_with(|| self.sort_key_2.cmp(&other.sort_key_2))
            .then_with(|| self.sort_key_3.cmp(&other.sort_key_3))
            .then_with(|| self.payload.cmp(&other.payload))
            .then_with(|| self.rule.cmp(&other.rule))
    }
}

impl PartialOrd for SortedRuleItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn get_sorted_rules(rules: &HashMap<String, String>) -> Vec<String> {
    let mut items = Vec::new();
    for (payload, rule) in rules {
        let rule_type = extract_type(rule);
        let rank = get_rank(&rule_type);
        if is_semantic_type(&rule_type) {
            items.push(SortedRuleItem {
                payload: payload.clone(),
                rule: rule.clone(),
                sort_key_0: 0,
                sort_key_1: payload.matches('.').count(),
                sort_key_2: payload.len(),
                sort_key_3: rank,
            });
        } else {
            items.push(SortedRuleItem {
                payload: payload.clone(),
                rule: rule.clone(),
                sort_key_0: 1,
                sort_key_1: rank as usize,
                sort_key_2: 0,
                sort_key_3: 0,
            });
        }
    }
    items.sort();
    items.into_iter().map(|item| item.rule).collect()
}

fn load_list(filepath: &Path) -> HashMap<String, String> {
    let mut rules = HashMap::new();
    if !filepath.exists() {
        return rules;
    }
    if let Ok(content) = fs::read_to_string(filepath) {
        for raw_line in content.lines() {
            let line = clean_rule(raw_line);
            if line.is_empty() || !is_valid_rule(&line) {
                continue;
            }
            let payload = extract_payload(&line);
            if payload.is_empty() {
                continue;
            }
            rules.entry(payload)
                .and_modify(|existing| *existing = prefer_rule(existing, &line))
                .or_insert(line);
        }
    }
    rules
}

fn write_list(filepath: &Path, rules: &HashMap<String, String>) {
    let sorted_rules = get_sorted_rules(rules);
    let mut existing_header = Vec::new();

    if filepath.exists() {
        if let Ok(content) = fs::read_to_string(filepath) {
            for line in content.lines() {
                if line.trim().starts_with('#') || line.trim().is_empty() {
                    existing_header.push(line.to_string());
                } else {
                    break;
                }
            }
        }
    }

    let mut out_lines = Vec::new();
    if !existing_header.is_empty() {
        for line in existing_header {
            if HEADER_RE.is_match(&line) {
                let key = line.splitn(2, ':').next().unwrap_or(&line);
                out_lines.push(format!("{}: {}\n", key, sorted_rules.len()));
            } else {
                out_lines.push(format!("{}\n", line));
            }
        }
        if let Some(last) = out_lines.last() {
            if !last.trim().is_empty() {
                out_lines.push("\n".to_string());
            }
        }
    } else {
        let file_name = filepath.file_name().unwrap_or_default().to_string_lossy();
        out_lines.push(format!("# Ruleset: {}\n", file_name));
        out_lines.push("# Cleaned by smart_cleanup.rs via Rust FFI\n\n".to_string());
    }

    for rule in sorted_rules {
        out_lines.push(format!("{}\n", rule));
    }

    let final_content = out_lines.join("");
    let _ = crate::safe_write_file_internal(filepath, &final_content, true);
}

pub fn run_cleanup(root_dir: &str) -> HashMap<String, i32> {
    let root = Path::new(root_dir);
    let ruleset_dirs = vec![
        root.join("rulesets").join("RULE-SET"),
        root.join("rulesets").join("AdBlock"),
    ];

    let exempt_filenames: HashSet<&str> = [
        "HTTPDNS_Hijack.list",
        "reject-drop.list",
        "reject-no-drop.list",
        "AdBlock.list",
    ].iter().cloned().collect();

    let mut file_content = HashMap::new();
    let mut file_paths = HashMap::new();
    let mut adblock_payloads = HashSet::new();
    let mut stats = HashMap::new();

    stats.insert("files".to_string(), 0);
    stats.insert("within_file_dedup".to_string(), 0);
    stats.insert("moved_direct_to_proxy".to_string(), 0);
    stats.insert("moved_proxy_to_direct".to_string(), 0);
    stats.insert("nsfw_purged_from_social".to_string(), 0);
    stats.insert("cross_file_removed".to_string(), 0);
    stats.insert("semantic_cover_removed".to_string(), 0);
    stats.insert("direct_proxyish_remaining".to_string(), 0);
    stats.insert("globalproxy_cn_remaining".to_string(), 0);

    // 1. Load lists
    for dir in &ruleset_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            let mut filenames = Vec::new();
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if !file_type.is_dir() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if name_str.ends_with(".list") {
                            filenames.push(name_str.into_owned());
                        }
                    }
                }
            }
            filenames.sort();

            for filename in filenames {
                if exempt_filenames.contains(filename.as_str()) {
                    continue;
                }
                let filepath = dir.join(&filename);
                let raw_unique = load_list(&filepath);
                file_content.insert(filename.clone(), raw_unique.clone());
                file_paths.insert(filename.clone(), filepath);

                for pfx in ADBLOCK_ONLY_PREFIXES {
                    if filename.starts_with(pfx) {
                        for k in raw_unique.keys() {
                            adblock_payloads.insert(k.clone());
                        }
                        break;
                    }
                }
            }
        }
    }
    stats.insert("files".to_string(), file_content.len() as i32);

    // 2. Rebalance generic rules
    let mut direct = file_content.remove("Direct.list").unwrap_or_default();
    let mut proxy = file_content.remove("GlobalProxy.list").unwrap_or_default();

    let mut moved_direct_to_proxy = 0;
    let mut direct_keys: Vec<String> = direct.keys().cloned().collect();
    for payload in direct_keys {
        if let Some(rule) = direct.get(&payload) {
            if is_proxyish_domain(rule) {
                proxy.insert(payload.clone(), rule.clone());
                direct.remove(&payload);
                moved_direct_to_proxy += 1;
            }
        }
    }
    stats.insert("moved_direct_to_proxy".to_string(), moved_direct_to_proxy);

    let mut moved_proxy_to_direct = 0;
    let proxy_keys: Vec<String> = proxy.keys().cloned().collect();
    for payload in proxy_keys {
        if let Some(rule) = proxy.get(&payload) {
            if is_cn_tld(rule) && !is_proxyish_domain(rule) {
                direct.insert(payload.clone(), rule.clone());
                proxy.remove(&payload);
                moved_proxy_to_direct += 1;
            }
        }
    }
    stats.insert("moved_proxy_to_direct".to_string(), moved_proxy_to_direct);

    file_content.insert("Direct.list".to_string(), direct);
    file_content.insert("GlobalProxy.list".to_string(), proxy);

    // 3. Purge NSFW from social
    let mut nsfw_purged = 0;
    if let Some(social) = file_content.get_mut("SocialMedia.list") {
        let social_keys: Vec<String> = social.keys().cloned().collect();
        for payload in social_keys {
            for kw in NSFW_CONTAMINATION_KEYWORDS {
                if payload.contains(kw) {
                    social.remove(&payload);
                    nsfw_purged += 1;
                    break;
                }
            }
        }
    }
    stats.insert("nsfw_purged_from_social".to_string(), nsfw_purged);

    // 4. Deduplicate by priority
    let mut all_lists: Vec<String> = file_content.keys().cloned().collect();
    all_lists.sort();

    let fallback_rulesets: HashSet<&str> = [
        "Direct.list",
        "Direct_ip.list",
        "GlobalProxy.list",
        "GlobalProxy_ip.list",
    ].iter().cloned().collect();

    let mut fallback_files = Vec::new();
    let mut specific_files = Vec::new();
    for f in &all_lists {
        if fallback_rulesets.contains(f.as_str()) {
            fallback_files.push(f.clone());
        } else {
            specific_files.push(f.clone());
        }
    }

    let mut expanded_specific = Vec::new();
    let mut seen_specific = HashSet::new();
    for p in PRIORITY_ORDER {
        let is_fb_prefix = fallback_rulesets.iter().any(|fb| fb.starts_with(p));
        if is_fb_prefix {
            continue;
        }
        for filename in &specific_files {
            if filename.starts_with(p) && !seen_specific.contains(filename) {
                expanded_specific.push(filename.clone());
                seen_specific.insert(filename.clone());
            }
        }
    }
    let mut remaining_specific = Vec::new();
    for f in &specific_files {
        if !seen_specific.contains(f) {
            remaining_specific.push(f.clone());
        }
    }
    let mut final_specific = expanded_specific;
    final_specific.extend(remaining_specific);

    let mut expanded_fallback = Vec::new();
    let mut seen_fallback = HashSet::new();
    for p in PRIORITY_ORDER {
        for filename in &fallback_files {
            if filename.starts_with(p) && !seen_fallback.contains(filename) {
                expanded_fallback.push(filename.clone());
                seen_fallback.insert(filename.clone());
            }
        }
    }
    let mut remaining_fallback = Vec::new();
    for f in &fallback_files {
        if !seen_fallback.contains(f) {
            remaining_fallback.push(f.clone());
        }
    }
    let mut final_fallback = expanded_fallback;
    final_fallback.extend(remaining_fallback);

    let mut priority = final_specific;
    priority.extend(final_fallback);

    let mut effective_semantic_coverage = HashSet::new();
    for p in SEMANTIC_COVERAGE_RULESETS {
        for filename in &all_lists {
            if filename.starts_with(p) {
                effective_semantic_coverage.insert(filename.clone());
            }
        }
    }

    let mut seen_payloads = HashSet::new();
    let mut seen_covering_suffixes = HashSet::new();
    let mut cross_file_removed = 0;
    let mut semantic_cover_removed = 0;

    for filename in &priority {
        if let Some(rules) = file_content.get_mut(filename) {
            let mut kept = HashMap::new();
            let mut sorted_keys: Vec<String> = rules.keys().cloned().collect();
            sorted_keys.sort();

            for payload in sorted_keys {
                if let Some(rule) = rules.get(&payload) {
                    let is_adblock_prefix = ADBLOCK_ONLY_PREFIXES.iter().any(|pfx| filename.starts_with(pfx));
                    if !is_adblock_prefix && adblock_payloads.contains(&payload) {
                        semantic_cover_removed += 1;
                        continue;
                    }

                    let rule_type = extract_type(rule);
                    if seen_payloads.contains(&payload) {
                        cross_file_removed += 1;
                        continue;
                    }

                    if is_semantic_type(&rule_type) {
                        let mut covered = false;
                        for parent in iter_domain_parents(&payload) {
                            if seen_covering_suffixes.contains(&parent) {
                                covered = true;
                                break;
                            }
                        }
                        if covered {
                            semantic_cover_removed += 1;
                            continue;
                        }
                    }

                    seen_payloads.insert(payload.clone());
                    kept.insert(payload.clone(), rule.clone());
                    if effective_semantic_coverage.contains(filename) && rule_type == "DOMAIN-SUFFIX" {
                        seen_covering_suffixes.insert(payload);
                    }
                }
            }
            *rules = kept;
        }
    }

    stats.insert("cross_file_removed".to_string(), cross_file_removed);
    stats.insert("semantic_cover_removed".to_string(), semantic_cover_removed);

    // 5. Audit
    let mut direct_proxyish_remaining = 0;
    if let Some(direct) = file_content.get("Direct.list") {
        for rule in direct.values() {
            if is_proxyish_domain(rule) {
                direct_proxyish_remaining += 1;
            }
        }
    }
    stats.insert("direct_proxyish_remaining".to_string(), direct_proxyish_remaining);

    let mut globalproxy_cn_remaining = 0;
    if let Some(proxy) = file_content.get("GlobalProxy.list") {
        for rule in proxy.values() {
            if is_cn_tld(rule) && !is_proxyish_domain(rule) {
                globalproxy_cn_remaining += 1;
            }
        }
    }
    stats.insert("globalproxy_cn_remaining".to_string(), globalproxy_cn_remaining);

    // 6. Save
    for (filename, rules) in &file_content {
        if let Some(filepath) = file_paths.get(filename) {
            if rules.is_empty() && filename != "AdBlock.list" {
                if filepath.exists() {
                    let _ = fs::remove_file(filepath);
                    println!("\x1b[0;32m[✓]\x1b[0m Deleted empty ruleset file: {}", filename);
                }
            } else {
                write_list(filepath, rules);
            }
        }
    }

    for dir in &ruleset_dirs {
        let readme = dir.join("README.md");
        if readme.exists() {
            let _ = fs::remove_file(&readme);
            println!("\x1b[0;32m[✓]\x1b[0m Deleted stray document: {:?}", readme);
        }
    }

    stats
}
