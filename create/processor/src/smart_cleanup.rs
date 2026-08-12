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
    if stripped.to_ascii_uppercase().starts_with("URL-REGEX,") {
        return normalize_url_regex_rule(&stripped).unwrap_or_default();
    }
    if stripped.starts_with("DOMAIN") && stripped.contains(",no-resolve") {
        stripped.replace(",no-resolve", "")
    } else {
        stripped
    }
}

fn normalize_url_regex_rule(line: &str) -> Option<String> {
    let raw = line.split_once(',')?.1.trim();
    if let Some(value) = raw.strip_prefix("\"\"") {
        let body = value.split_once("\",").map_or(value, |(body, _)| body);
        return Some(format!("URL-REGEX,{}", body));
    }
    if let Some(quoted) = raw.strip_prefix('"') {
        let end = quoted.find('"')?;
        return Some(format!("URL-REGEX,{}", &quoted[..end]));
    }
    Some(format!("URL-REGEX,{}", raw.split(',').next()?.trim()))
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
    let direct_keys: Vec<String> = direct.keys().cloned().collect();
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
        if dir.file_name().is_some_and(|name| name == "AdBlock") {
            continue;
        }
        let readme = dir.join("README.md");
        if readme.exists() {
            let _ = fs::remove_file(&readme);
            println!("\x1b[0;32m[✓]\x1b[0m Deleted stray document: {:?}", readme);
        }
    }

    stats
}

#[derive(Clone, Debug)]
#[allow(non_snake_case)]
pub struct ModuleSectionPub {
    pub Name: String,
    pub Lines: Vec<String>,
}

static AD_SCRIPT_MARKERS: &[&str] = &[
    "adblock", "/adblock/", ".adblock.", "anti-ad", "blockad", "remove_ads",
    "remove-ads", "_remove_", "/remove", "去广告", "/ads/", "advertising", "reject",
    "spam", "tracking", "xwebads", "adultraplus", "allinone", "mock",
    "bilibili.helper", "helper.v2.js", "camoufox", "rednote", "redpaper",
    "weibo_ads", "weibo_main", "wool_scripts", "kelee.one/resource/javascript",
    "proxyrules/main/script", "ithome_remove", "zhihu_remove", "amap_remove",
    "cainiao_remove", "taobao_remove", "jd_remove", "spotify_remove", "bilicomic_remove",
];

static AD_ENTRY_NAME_MARKERS: &[&str] = &[
    "去广告", "adblock", "anti-ad", "xwebads", "移除", "精简", "推广", "开屏", "净化", "广告", "killad", "mock",
];

static ENHANCE_SCRIPT_MARKERS: &[&str] = &[
    "/enhanced/", ".enhanced.", "/enhance/", "biliuniverse/enhanced", "/global/",
    ".global.", "biliuniverse/global", "/redirect/", ".redirect.", "unlock", "解锁",
    "unblock", "vip", "premium", "crack", "破解", "camscanner.js", "external_links_unlock",
    "weixin_external", "1080p", "bilibili_json.js", "dualsubs", "iringo", "weatherkit",
    "zheye", "哲也", "translation", "翻译", "nsfw", "region", "intl", "bypass", "绕过",
    "pip", "画中画", "optimize", "功能增强",
];

static ENHANCE_NAME_PREFIXES: &[&str] = &[
    "bilibili.enhanced", "bilibili.global", "bilibili.redirect", "youtube.enhance",
    "wechat_enhance", "iringo.", "dualsubs",
];

static AD_NAME_PREFIXES: &[&str] = &[
    "bilibili.adblock", "adblock", "去广告", "xwebads", "anti-ad",
];

static REJECT_POLICIES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.insert("REJECT");
    s.insert("REJECT-DROP");
    s.insert("REJECT-NO-DROP");
    s.insert("REJECT-TINYGIF");
    s.insert("REJECT-IMG");
    s
});

static PURE_AD_MODULE_NAME_TOKENS: &[&str] = &[
    "去广告", "adblock", "anti-ad", "xwebads", "adultraplus", "allinone", "all-in-one",
    "rednote", "redpaper", "微博去广告", "remove_ads",
];

static SCRIPT_PATH_RE_STR: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)script-path\s*=\s*([^,\s]+)").unwrap());
static SCRIPT_NAME_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bname\s*=\s*([^,\s]+)").unwrap());
static SCRIPT_LABEL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(.+?)\s*=\s*type=").unwrap());
static SCRIPT_PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bscript-path\s*=\s*([^,\s]+)").unwrap());
static SCRIPT_PATTERN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bpattern\s*=\s*([^,]+)").unwrap());
static SECTION_HEADER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[([^\]]+)\]\s*$").unwrap());

fn script_path(line: &str) -> String {
    if let Some(m) = SCRIPT_PATH_RE_STR.captures(line) {
        return m[1].to_lowercase();
    }
    String::new()
}

fn entry_name(line: &str) -> String {
    if !line.contains('=') {
        return String::new();
    }
    let parts: Vec<&str> = line.splitn(2, '=').collect();
    parts[0].trim().to_lowercase()
}

fn contains_any(s: &str, markers: &[&str]) -> bool {
    for &m in markers {
        if s.contains(m) {
            return true;
        }
    }
    false
}

pub fn source_is_pure_ad_module_pub(source_path: &str) -> bool {
    if source_path.is_empty() {
        return false;
    }
    let filename = Path::new(source_path).file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let low = filename.to_lowercase();
    contains_any(&low, PURE_AD_MODULE_NAME_TOKENS)
}

fn script_is_ad(name: &str, spath: &str) -> bool {
    if contains_any(name, AD_NAME_PREFIXES) || contains_any(name, AD_ENTRY_NAME_MARKERS) {
        if contains_any(name, &["解锁", "unlock", "vip", "premium", "破解", "crack"]) {
            return false;
        }
        return true;
    }
    if contains_any(spath, AD_SCRIPT_MARKERS) {
        return true;
    }
    if name.contains("helper") && name.contains("bili") {
        return true;
    }
    false
}

fn script_is_enhance(name: &str, spath: &str) -> bool {
    if contains_any(name, ENHANCE_NAME_PREFIXES) || contains_any(spath, ENHANCE_SCRIPT_MARKERS) {
        return true;
    }
    if contains_any(name, &["解锁", "unlock", "vip", "premium", "破解", "crack"]) {
        return true;
    }
    #[allow(dead_code)]
    #[derive(Clone, Debug)]
    enum Verdict { Ad, Enhance, Neutral }
    false
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineVerdict {
    Ad,
    Enhance,
    Neutral,
}

pub fn classify_promax_line_pub(line: &str, section: &str, source_path: &str) -> LineVerdict {
    let stripped = line.trim();
    if stripped.is_empty() || stripped.starts_with('#') {
        return LineVerdict::Neutral;
    }

    let low = stripped.to_lowercase();
    let sec = section.trim().to_lowercase();
    let pure_ad_source = source_is_pure_ad_module_pub(source_path);

    if sec == "mitm" {
        if !low.contains("hostname") {
            return LineVerdict::Neutral;
        }
        if contains_any(&low, &["unlock", "解锁", "camscanner", "intsig.net"]) {
            return LineVerdict::Enhance;
        }
        return LineVerdict::Neutral;
    }

    if sec == "general" || sec == "header rewrite" {
        return LineVerdict::Enhance;
    }

    let name = entry_name(stripped);
    let spath = script_path(stripped);

    if sec == "script" || low.contains("script-path=") {
        if script_is_enhance(&name, &spath) {
            return LineVerdict::Enhance;
        }
        if script_is_ad(&name, &spath) {
            return LineVerdict::Ad;
        }
        if pure_ad_source {
            return LineVerdict::Ad;
        }
        return LineVerdict::Enhance;
    }

    if sec == "url rewrite" || (stripped.starts_with('^') && (low.contains(" reject") || low.contains(" _ reject"))) {
        if low.contains(" reject") || low.contains(" _ reject") {
            return LineVerdict::Ad;
        }
        return LineVerdict::Enhance;
    }

    if sec == "map local" || sec == "body rewrite" || sec == "maplocal" {
        if low.contains("http-response-jq") || low.contains("http-request-jq") {
            if contains_any(&low, ENHANCE_SCRIPT_MARKERS) {
                return LineVerdict::Enhance;
            }
            return LineVerdict::Ad;
        }
        if stripped.starts_with("http-response ") || stripped.starts_with("http-request ") {
            return LineVerdict::Ad;
        }
        if stripped.starts_with('^') && (low.contains("data-type=") || low.contains("data=\"{}\"") || low.contains("data='{}'")) {
            return LineVerdict::Ad;
        }
        if contains_any(&low, &["广告", "adcard", "splash", "deliver", "flash", "e-commerce"]) {
            return LineVerdict::Ad;
        }
        if low.contains("data=\"{}\"") || low.contains("data='{}'") {
            return LineVerdict::Ad;
        }
        if low.contains("del(.data.payment)") || (low.contains("payment") && low.contains("del(")) {
            return LineVerdict::Ad;
        }
        if pure_ad_source && stripped.starts_with('^') {
            return LineVerdict::Ad;
        }
        return LineVerdict::Neutral;
    }

    let parts: Vec<&str> = stripped.split(',').collect();
    let first_upper = parts[0].trim().to_uppercase();
    if sec == "rule" || ["DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "DOMAIN-REGEX", "DOMAIN-WILDCARD", "URL-REGEX", "IP-CIDR", "IP-CIDR6", "USER-AGENT", "PROCESS-NAME", "DEST-PORT"].contains(&first_upper.as_str()) {
        let mut policy = String::new();
        if !parts.is_empty() {
            policy = parts[parts.len()-1].trim().to_uppercase();
        }
        if REJECT_POLICIES.contains(policy.as_str()) {
            if contains_any(&low, &["unlock", "解锁", "vip", "premium", "crack"]) {
                return LineVerdict::Enhance;
            }
            return LineVerdict::Ad;
        }
        if !parts.is_empty() {
            if ["DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "DOMAIN-REGEX", "DOMAIN-WILDCARD", "URL-REGEX", "IP-CIDR", "IP-CIDR6", "USER-AGENT", "PROCESS-NAME", "DEST-PORT"].contains(&first_upper.as_str()) {
                if contains_any(&low, &["unlock", "解锁", "vip", "premium", "crack"]) {
                    return LineVerdict::Enhance;
                }
                return LineVerdict::Ad;
            }
        }
        return LineVerdict::Enhance;
    }

    if contains_any(&low, ENHANCE_SCRIPT_MARKERS) {
        return LineVerdict::Enhance;
    }
    if contains_any(&low, AD_SCRIPT_MARKERS) {
        return LineVerdict::Ad;
    }
    LineVerdict::Neutral
}

pub fn should_keep_promax_line_pub(line: &str, section: &str, source_path: &str) -> bool {
    let verdict = classify_promax_line_pub(line, section, source_path);
    if verdict == LineVerdict::Ad {
        return true;
    }
    if verdict == LineVerdict::Enhance {
        return false;
    }
    let sec = section.trim().to_lowercase();
    if sec == "mitm" && line.to_lowercase().contains("hostname") {
        return true;
    }
    false
}

pub fn module_ingest_mode_pub(path: &str, text: &str) -> &'static str {
    let filename = Path::new(path).file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = filename.to_lowercase();
    if name.contains("解锁") || name.contains("unlock") {
        if !contains_any(&name, &["去广告", "adblock", "anti-ad"]) {
            return "skip";
        }
    }

    let mut ad_lines = 0;
    let mut enhance_lines = 0;
    let mut current_section = String::new();

    for line in text.lines() {
        let stripped = line.trim();
        if stripped.starts_with('[') && stripped.ends_with(']') {
            current_section = stripped[1..stripped.len()-1].to_string();
            continue;
        }
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        let v = classify_promax_line_pub(stripped, &current_section, path);
        if v == LineVerdict::Ad {
            ad_lines += 1;
        } else if v == LineVerdict::Enhance {
            enhance_lines += 1;
        }
    }

    if ad_lines == 0 {
        return "skip";
    }
    if enhance_lines > 0 {
        return "split";
    }
    return "full";
}

pub fn dedupe_key_pub(section: &str, line: &str) -> String {
    let stripped = line.trim();
    if stripped.is_empty() || stripped.starts_with('#') {
        return String::new();
    }
    if section == "Script" {
        if let Some(caps) = SCRIPT_LABEL_RE.captures(stripped) {
            return format!("script:label:{}", caps[1].trim());
        }
        if let Some(caps) = SCRIPT_NAME_RE.captures(stripped) {
            return format!("script:name:{}", &caps[1]);
        }
        let path_opt = SCRIPT_PATH_RE.captures(stripped).map(|c| c[1].to_string());
        let pattern_opt = SCRIPT_PATTERN_RE.captures(stripped).map(|c| c[1].trim().to_string());
        if let (Some(path), Some(pattern)) = (path_opt, pattern_opt) {
            return format!("script:{}:{}", path, pattern);
        }
        if let Some(caps) = SCRIPT_PATH_RE.captures(stripped) {
            return format!("script:{}", &caps[1]);
        }
        return format!("script:{}", stripped);
    }
    if section == "URL Rewrite" {
        let parts: Vec<&str> = stripped.splitn(2, ',').collect();
        return format!("rewrite:{}", parts[0]);
    }
    if section == "Map Local" {
        return format!("maplocal:{}", stripped);
    }
    if section == "Body Rewrite" {
        return format!("body:{}", stripped);
    }
    if section == "Header Rewrite" {
        return format!("header:{}", stripped);
    }
    if section == "Rule" {
        return format!("rule:{}", stripped);
    }
    if section == "MITM" {
        let re = Regex::new(r"(?i)^hostname\s*=\s*(%APPEND%\s*)?").unwrap();
        let hosts = re.replace_all(stripped, "");
        return format!("mitm:{}", hosts.trim());
    }
    format!("{}:{}", section, stripped)
}

pub fn dedupe_section_lines_pub(section: &str, lines: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result: Vec<String> = Vec::new();
    for line in lines {
        let stripped = line.trim();
        if stripped.is_empty() {
            if !result.is_empty() && !result.last().unwrap().is_empty() {
                result.push(String::new());
            }
            continue;
        }
        if stripped.starts_with('#') {
            result.push(stripped.to_string());
            continue;
        }
        let key = dedupe_key_pub(section, line);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        result.push(stripped.to_string());
    }

    if ["Script", "Host", "MITM", "General"].contains(&section) {
        #[derive(Clone)]
        struct ParsedLine {
            raw: String,
            left: String,
            right: String,
        }
        let mut parsed = Vec::new();
        let mut max_len = 0;
        for r in &result {
            if r.starts_with('#') {
                parsed.push(ParsedLine { raw: r.clone(), left: String::new(), right: String::new() });
                continue;
            }
            let parts: Vec<&str> = r.splitn(2, '=').collect();
            if parts.len() == 2 {
                let left = parts[0].trim().to_string();
                let right = parts[1].trim().to_string();
                if left.len() < 60 && left.len() > max_len {
                    max_len = left.len();
                }
                parsed.push(ParsedLine { raw: r.clone(), left, right });
            } else {
                parsed.push(ParsedLine { raw: r.clone(), left: String::new(), right: String::new() });
            }
        }
        let mut aligned_result = Vec::new();
        for p in parsed {
            if !p.left.is_empty() && !p.right.is_empty() && p.left.len() <= 60 {
                aligned_result.push(format!("{:<width$} = {}", p.left, p.right, width = max_len));
            } else {
                aligned_result.push(p.raw);
            }
        }
        return aligned_result;
    }
    result
}

static HEADER_KEYS_ORDER: &[&str] = &[
    "name", "desc", "author", "icon", "category", "tag", "date", "arguments", "arguments-desc", "system-proxy", "ability", "update-interval",
];

pub fn format_header_pub(meta: &HashMap<String, String>) -> Vec<String> {
    let mut lines = Vec::new();
    let mut used = HashSet::new();
    for &key in HEADER_KEYS_ORDER {
        if let Some(val) = meta.get(key) {
            lines.push(format!("#!{}={}", key, val));
            used.insert(key);
        }
    }

    let mut remaining = Vec::new();
    for key in meta.keys() {
        if !used.contains(key.as_str()) {
            remaining.push(key.clone());
        }
    }
    remaining.sort();
    for key in remaining {
        lines.push(format!("#!{}={}", key, meta.get(&key).unwrap()));
    }
    lines
}

static SECTION_ORDER: &[&str] = &[
    "General", "Host", "Rule", "URL Rewrite", "Map Local", "Script", "Body Rewrite", "Header Rewrite", "MITM",
];

pub fn format_module_pub(header_lines: &[String], sections: &[ModuleSectionPub], dedupe: bool) -> String {
    let mut section_map = HashMap::new();
    for sec in sections {
        section_map.insert(sec.Name.clone(), sec.Lines.clone());
    }

    let mut out = header_lines.to_vec();
    while !out.is_empty() && out.last().unwrap().trim().is_empty() {
        out.pop();
    }
    if !out.is_empty() {
        out.push(String::new());
    }

    for &section_name in SECTION_ORDER {
        if let Some(lines) = section_map.get(section_name) {
            if lines.is_empty() {
                continue;
            }
            let lines = if dedupe {
                dedupe_section_lines_pub(section_name, lines)
            } else {
                lines.clone()
            };
            if lines.is_empty() {
                continue;
            }
            out.push(format!("[{}]", section_name));
            out.extend(lines);
            out.push(String::new());
        }
    }

    let mut res = out.join("\n");
    while res.ends_with('\n') {
        res.pop();
    }
    res.push('\n');
    res
}

pub fn split_module_sections_pub(path: &str, text: &str) -> (HashMap<String, Vec<String>>, HashMap<String, usize>) {
    let mut ad_sections = HashMap::new();
    let mut current_section = String::new();
    let mut total = 0;
    let mut kept = 0;
    let mut skipped = 0;

    for line in text.lines() {
        let stripped = line.trim();
        if let Some(m) = SECTION_HEADER_RE.captures(stripped) {
            if !stripped.starts_with('#') {
                current_section = m[1].to_string();
                continue;
            }
        }
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        if current_section.is_empty() {
            continue;
        }
        let sec_lower = current_section.to_lowercase();
        if sec_lower == "general" || sec_lower == "ponte" {
            skipped += 1;
            total += 1;
            continue;
        }
        total += 1;
        if should_keep_promax_line_pub(stripped, &current_section, path) {
            ad_sections.entry(current_section.clone()).or_insert_with(Vec::new).push(stripped.to_string());
            kept += 1;
        } else {
            skipped += 1;
        }
    }

    let mut stats = HashMap::new();
    stats.insert("total_lines".to_string(), total);
    stats.insert("kept_lines".to_string(), kept);
    stats.insert("skipped_lines".to_string(), skipped);

    (ad_sections, stats)
}

pub fn format_split_module_pub(source_path: &str, ad_sections: &HashMap<String, Vec<String>>, note: &str) -> String {
    let base = Path::new(source_path).file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut lines = vec![
        format!("#!name=PROMAX split · {}", base),
        format!("#!desc={}", note),
        "#!author=ScriptHub-Automated".to_string(),
        format!("#!split-source={}", source_path),
    ];

    let order = &["Rule", "URL Rewrite", "Map Local", "Script", "Body Rewrite", "Header Rewrite", "MITM"];

    for &sec in order {
        if let Some(body) = ad_sections.get(sec) {
            if body.is_empty() {
                continue;
            }
            lines.push(String::new());
            lines.push(format!("[{}]", sec));
            lines.extend(body.clone());
        }
    }

    let mut remaining = Vec::new();
    for sec in ad_sections.keys() {
        if !order.contains(&sec.as_str()) {
            remaining.push(sec.clone());
        }
    }
    remaining.sort();

    for sec in remaining {
        if let Some(body) = ad_sections.get(&sec) {
            lines.push(String::new());
            lines.push(format!("[{}]", sec));
            lines.extend(body.clone());
        }
    }
    lines.push(String::new());

    lines.join("\n")
}

pub fn adapt_rewrite_line_for_loon_pub(line: &str) -> String {
    let stripped = line.trim();
    if stripped.is_empty() || stripped.starts_with('#') {
        return line.to_string();
    }

    let lower = stripped.to_lowercase();
    let mut output = line.to_string();
    if lower.contains(" _ reject") {
        output = output.replace(" _ reject", " reject");
    } else if lower.contains(" - reject") {
        output = output.replace(" - reject", " reject");
    } else if lower.contains(" - reject-200") {
        output = output.replace(" - reject-200", " reject-200");
    } else if lower.contains(" - reject-img") {
        output = output.replace(" - reject-img", " reject-img");
    } else if lower.contains(" - reject-tinygif") {
        output = output.replace(" - reject-tinygif", " reject-tinygif");
    } else if lower.contains(" - reject-no-drop") {
        output = output.replace(" - reject-no-drop", " reject-no-drop");
    } else if lower.contains(" - 302 ") {
        output = output.replace(" - 302 ", " 302 ");
    } else if lower.contains(" - 307 ") {
        output = output.replace(" - 307 ", " 307 ");
    }
    output
}

pub fn adapt_map_local_line_for_loon_pub(line: &str) -> String {
    let stripped = line.trim();
    if stripped.is_empty() || stripped.starts_with('#') {
        return line.to_string();
    }
    let Some((pattern, parameters)) = stripped.split_once(char::is_whitespace) else {
        return String::new();
    };
    let mut converted = Vec::new();
    let mut data_path = None;
    for field in split_whitespace_parameters(parameters) {
        let lower = field.to_ascii_lowercase();
        if lower.starts_with("header=") {
            continue;
        }
        if lower.starts_with("data=") {
            let value = field[5..].trim();
            if value.trim_matches('"').starts_with("http") {
                data_path = Some(value.to_string());
                continue;
            }
        }
        converted.push(field.to_string());
    }
    if let Some(path) = data_path {
        if !converted.iter().any(|field| field.starts_with("data-type=")) {
            let unquoted = path.trim_matches('"').to_ascii_lowercase();
            let data_type = if unquoted.ends_with(".json") { "json" } else { "text" };
            converted.push(format!("data-type={data_type}"));
        }
        converted.push(format!("data-path={path}"));
    }
    if !converted.iter().any(|field| field.starts_with("status-code=")) {
        converted.push("status-code=200".to_string());
    }
    if !converted.iter().any(|field| {
        field.starts_with("data=") || field.starts_with("data-path=")
    }) {
        return String::new();
    }
    format!("{pattern} mock-response-body {}", converted.join(" "))
}

pub fn adapt_body_rewrite_line_for_loon_pub(line: &str) -> String {
    let stripped = line.trim();
    if stripped.is_empty() || stripped.starts_with('#') {
        return line.to_string();
    }
    let mut fields = stripped.splitn(3, char::is_whitespace);
    let operation = fields.next().unwrap_or("");
    let pattern = fields.next().unwrap_or("");
    let arguments = fields.next().unwrap_or("");
    let loon_operation = match operation {
        "http-response-jq" => "response-body-json-jq",
        "http-request-jq" => "request-body-json-jq",
        "http-response" => "response-body-replace-regex",
        "http-request" => "request-body-replace-regex",
        _ => return String::new(),
    };
    if pattern.is_empty() || arguments.is_empty() {
        String::new()
    } else {
        format!("{pattern} {loon_operation} {arguments}")
    }
}

pub fn adapt_script_line_for_loon_pub(tag: &str, line_content: &str) -> String {
    let mut params = HashMap::new();
    for p in split_script_parameters(line_content) {
        if let Some(idx) = p.find('=') {
            let k = p[..idx].trim().to_lowercase();
            let v = p[idx+1..].trim().to_string();
            params.insert(k, v);
        }
    }

    let script_type = params.get("type").map(|s| s.as_str()).unwrap_or("http-response");
    let script_path = match params.get("script-path") {
        Some(p) => p,
        None => return String::new(),
    };

    let prefix = match script_type {
        "http-request" | "http-response" => match params.get("pattern") {
            Some(pattern) => format!("{} {}", script_type, pattern),
            None => return String::new(),
        },
        "cron" => {
            let expression = params
                .get("cronexp")
                .or_else(|| params.get("cron-expression"))
                .or_else(|| params.get("cron"));
            match expression {
                Some(expression) => format!("cron {}", quote_if_needed(expression)),
                None => return String::new(),
            }
        }
        "network-changed" | "generic" => script_type.to_string(),
        _ => return String::new(),
    };

    let mut loon_parts = Vec::new();
    loon_parts.push(format!("script-path={}", script_path));
    loon_parts.push(format!("tag={}", tag));

    if let Some(req_body) = params.get("requires-body") {
        if req_body == "1" || req_body == "true" {
            loon_parts.push("requires-body=true".to_string());
        }
    }
    if let Some(timeout) = params.get("timeout") {
        loon_parts.push(format!("timeout={}", timeout));
    }
    if let Some(binary_mode) = params.get("binary-body-mode") {
        loon_parts.push(format!("binary-body-mode={}", binary_mode));
    }
    if let Some(argument) = params.get("argument") {
        loon_parts.push(format!("argument={}", quote_if_needed(argument)));
    }

    format!("{} {}", prefix, loon_parts.join(","))
}

fn split_script_parameters(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut braces = 0_u32;
    let mut brackets = 0_u32;
    let mut parentheses = 0_u32;

    for (index, character) in input.char_indices() {
        if let Some(delimiter) = quote {
            if character == delimiter && !escaped {
                quote = None;
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.saturating_sub(1),
            ',' if braces == 0 && brackets == 0 && parentheses == 0 => {
                parts.push(input[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(input[start..].trim());
    parts
}

fn split_whitespace_parameters(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in input.char_indices() {
        if let Some(delimiter) = quote {
            if character == delimiter && !escaped {
                quote = None;
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            value if value.is_whitespace() => {
                if start < index {
                    parts.push(input[start..index].trim());
                }
                start = index + value.len_utf8();
            }
            _ => {}
        }
    }
    if start < input.len() {
        parts.push(input[start..].trim());
    }
    parts.into_iter().filter(|field| !field.is_empty()).collect()
}

fn quote_if_needed(value: &str) -> String {
    let value = value.trim();
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', r#"\""#))
    }
}

pub fn merge_mitm_hosts_pub(sections: &[ModuleSectionPub]) -> Vec<ModuleSectionPub> {
    let mut hosts = HashSet::new();
    let mut other = Vec::new();
    let skip_tokens: HashSet<&str> = ["%INSERT%", "%APPEND%"].iter().cloned().collect();

    for sec in sections {
        if sec.Name != "MITM" {
            other.push(sec.clone());
            continue;
        }
        for line in &sec.Lines {
            let stripped = line.trim();
            if stripped.to_lowercase().starts_with("hostname") {
                let re1 = Regex::new(r"(?i)^hostname\s*=\s*").unwrap();
                let part = re1.replace_all(stripped, "");
                let re2 = Regex::new(r"(?i)^(%APPEND%|%INSERT%)\s*").unwrap();
                let part = re2.replace_all(&part, "");
                for token in part.split(',') {
                    let token = token.trim();
                    if !token.is_empty() && !skip_tokens.contains(token) {
                        hosts.insert(token.to_string());
                    }
                }
            }
        }
    }

    if !hosts.is_empty() {
        let mut inclusions = Vec::new();
        let mut exclusions = Vec::new();
        for h in hosts {
            if h.starts_with('-') {
                exclusions.push(h);
            } else {
                inclusions.push(h);
            }
        }
        inclusions.sort();
        exclusions.sort();
        let mut merged = inclusions;
        merged.extend(exclusions);

        other.push(ModuleSectionPub {
            Name: "MITM".to_string(),
            Lines: vec![format!("hostname = %APPEND% {}", merged.join(", "))],
        });
    }
    other
}

#[cfg(test)]
mod ruleset_cleanup_tests {
    use super::normalize_url_regex_rule;

    #[test]
    fn normalizes_legacy_quoted_url_regex_policy() {
        assert_eq!(
            normalize_url_regex_rule(
                r#"URL-REGEX,""^https:\/\/ads\.example\/",REJECT-DROP"#,
            ),
            Some(r#"URL-REGEX,^https:\/\/ads\.example\/"#.to_string())
        );
    }
}

#[cfg(test)]
mod promax_loon_tests {
    use super::{
        adapt_body_rewrite_line_for_loon_pub, adapt_map_local_line_for_loon_pub,
        adapt_rewrite_line_for_loon_pub, adapt_script_line_for_loon_pub,
    };

    #[test]
    fn loon_url_rewrite_removes_surge_placeholder() {
        assert_eq!(
            adapt_rewrite_line_for_loon_pub(r#"^https://ads\.example _ reject"#),
            r#"^https://ads\.example reject"#
        );
    }

    #[test]
    fn loon_map_local_becomes_typed_mock_response() {
        assert_eq!(
            adapt_map_local_line_for_loon_pub(
                r#"^https://ads\.example data-type=text data="{}" status-code=200 header="Content-Type:application/json""#,
            ),
            r#"^https://ads\.example mock-response-body data-type=text data="{}" status-code=200"#
        );
        assert_eq!(
            adapt_map_local_line_for_loon_pub(
                r#"^https://ads\.example data="https://example.test/reject.json""#,
            ),
            r#"^https://ads\.example mock-response-body data-type=json data-path="https://example.test/reject.json" status-code=200"#
        );
    }

    #[test]
    fn loon_body_rewrite_moves_pattern_before_typed_operation() {
        assert_eq!(
            adapt_body_rewrite_line_for_loon_pub(
                r#"http-response-jq ^https:\/\/api\.example/ad 'del(.data.ad)'"#,
            ),
            r#"^https:\/\/api\.example/ad response-body-json-jq 'del(.data.ad)'"#
        );
        assert_eq!(
            adapt_body_rewrite_line_for_loon_pub(
                r#"http-response ^https:\/\/api\.example/ad banner replacement"#,
            ),
            r#"^https:\/\/api\.example/ad response-body-replace-regex banner replacement"#
        );
    }

    #[test]
    fn loon_script_preserves_regex_commas_json_argument_and_binary_mode() {
        let converted = adapt_script_line_for_loon_pub(
            "fixture",
            r#"type=http-response,pattern=^https://example\.com/a{1,4}/x,script-path=https://example.test/a.js,requires-body=1,binary-body-mode=true,argument="{\"a\":1,\"b\":2}""#,
        );

        assert!(converted.contains(r#"^https://example\.com/a{1,4}/x"#));
        assert!(converted.contains("binary-body-mode=true"));
        assert!(converted.contains(r#"argument="{\"a\":1,\"b\":2}""#));
    }
}
