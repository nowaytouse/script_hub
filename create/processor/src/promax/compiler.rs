use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

// Regex rules
#[allow(dead_code)]
static META_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^#!\s*([A-Za-z0-9_-]+)\s*[=:]\s*(.*)$").unwrap());
#[allow(dead_code)]
static SCRIPT_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bname\s*=\s*([^,\s]+)").unwrap());
#[allow(dead_code)]
static SCRIPT_LABEL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(.+?)\s*=\s*type=").unwrap());
#[allow(dead_code)]
static SCRIPT_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bscript-path\s*=\s*([^,\s]+)").unwrap());
#[allow(dead_code)]
static SCRIPT_PATTERN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bpattern\s*=\s*([^,]+)").unwrap());

#[allow(dead_code)]
static M1: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\(\^\|\.\)(.+?)-\.\+\.").unwrap());
#[allow(dead_code)]
static M2: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:\(\^\|\.\)|\^)([a-zA-Z0-9][-a-zA-Z0-9.]*)$").unwrap());
#[allow(dead_code)]
static M3: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\.\+(.+)$").unwrap());

#[allow(dead_code)]
static VALID_IPV4_CIDR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d{1,3}\.){3}\d{1,3}(/\d{1,2})?$").unwrap());
#[allow(dead_code)]
static VALID_IPV6_CIDR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([0-9a-fA-F:]+)(/\d{1,3})?$").unwrap());

static FUNCTIONAL_BLOCK_NAME_TOKENS: &[&str] = &[
    "解锁",
    "unlock",
    "unblock",
    "破解",
    "crack",
    "会员",
    "vip",
    "premium",
    "增强",
    "enhance",
    "enhanced",
    ".global",
    "global.sgmodule",
    "redirect",
    "翻译",
    "translation",
    "nsfw",
    "iringo",
    "weatherkit",
    "dualsubs",
    "sub_info",
    "boxjs",
    "timecard",
    "net-lsp",
    "surge-beta",
    "apple服务",
    "wechat_enhance",
    "reddit",
    "扫描全能",
    "camscanner",
    "nsringo",
    "geoservices",
    "boxjs",
    "maasea/sgmodule",
    "youtube.enhance",
    "增强合集",
    "bilibili.enhanced",
    "bilibili.global",
    "bilibili.redirect",
];

static ADBLOCK_MODULE_NAME_TOKENS: &[&str] = &[
    "去广告",
    "adblock",
    "anti-ad",
    "xwebads",
    "allinone",
    "all-in-one",
    "adultraplus",
    "微博去广告",
    "rednote",
    "redpaper",
];

static ADBLOCK_FILENAME_ALLOWLIST: &[&str] = &[
    "[Sukka] Enhance Better ADBlock for Surge.sgmodule",
    "sukka_url_redirect.sgmodule",
    "AllInOne_Mock.sgmodule",
    "All-in-One-2.x.sgmodule",
    "XWebAds.sgmodule",
    "BiliBili.ADBlock.sgmodule",
    "YouTube.ADBlock.sgmodule",
    "WeChat.ADBlock.sgmodule",
];

static FUNCTIONAL_BLOCK_LOCAL_SOURCES: &[&str] = &["Reddit.ADBlock.sgmodule"];

static CATEGORIES: &[&str] = &[
    "Local",
    "Advertising",
    "Privacy",
    "Security",
    "AntiAD",
    "ThreatIntel_Ultimate",
    "ThreatIntel_TIF",
    "ThreatIntel",
    "Other",
];

struct CategoryMeta {
    label_zh: &'static str,
    desc: &'static str,
}

static CATEGORY_META: Lazy<HashMap<&'static str, CategoryMeta>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(
        "Local",
        CategoryMeta {
            label_zh: "应用去广告",
            desc: "各 App 去广告域名规则（脚本/重写已并入 PROMAX，勿单独安装专项模块）",
        },
    );
    m.insert(
        "Advertising",
        CategoryMeta {
            label_zh: "通用广告",
            desc: "blackmatrix7 / ACL4SSR / geekdada / 广告联盟等通用域名",
        },
    );
    m.insert(
        "Privacy",
        CategoryMeta {
            label_zh: "隐私追踪",
            desc: "Privacy / tracking-protection / 防追踪列表",
        },
    );
    m.insert(
        "Security",
        CategoryMeta {
            label_zh: "安全威胁",
            desc: "劫持 / 钓鱼 / BanProgram / HTTPDNS / skk reject",
        },
    );
    m.insert(
        "AntiAD",
        CategoryMeta {
            label_zh: "Anti-AD",
            desc: "privacy-protection-tools anti-AD 主列表与 discretion",
        },
    );
    m.insert(
        "ThreatIntel_Ultimate",
        CategoryMeta {
            label_zh: "威胁情报·终极",
            desc: "HaGeZi DNS-Blocklists ultimate",
        },
    );
    m.insert(
        "ThreatIntel_TIF",
        CategoryMeta {
            label_zh: "威胁情报·TIF",
            desc: "HaGeZi TIF medium 威胁情报",
        },
    );
    m.insert(
        "ThreatIntel",
        CategoryMeta {
            label_zh: "威胁情报·补充",
            desc: "HaGeZi / skk 其它威胁与钓鱼域名集",
        },
    );
    m.insert(
        "Other",
        CategoryMeta {
            label_zh: "其他补充",
            desc: "社区 sgmodule / 未归类远程源",
        },
    );
    m
});
fn format_commas(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut res = String::new();
    let len = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            res.push(',');
        }
        res.push(b as char);
    }
    res
}

static SECTION_ORDER: &[&str] = &[
    "General",
    "Host",
    "Rule",
    "URL Rewrite",
    "Map Local",
    "Script",
    "Body Rewrite",
    "Header Rewrite",
    "MITM",
];

#[allow(dead_code)]
static HEADER_KEYS_ORDER: &[&str] = &[
    "name",
    "desc",
    "author",
    "icon",
    "category",
    "tag",
    "date",
    "arguments",
    "arguments-desc",
    "system-proxy",
    "ability",
    "update-interval",
];

static GROUP_HEAD_EXPANSE: &str = "『 🔝 Head Expanse › 首端扩域 』";
static CDN_BASE_URL: &str = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/";
static GITHUB_RAW_URL: &str = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/";
static RULES_PER_SHARD: usize = 100_000;

#[derive(Serialize, Deserialize)]
struct CatalogItem {
    path: String,
    install_url: String,
    url_source: String,
    note: String,
}

#[derive(Serialize, Deserialize)]
struct ShardItem {
    file: String,
    category: String,
    label_zh: String,
    purpose: String,
    shard_index: usize,
    rule_count: usize,
    cdn_url: String,
    github_url: String,
}

#[derive(Serialize, Deserialize)]
struct CatalogJson {
    updated: String,
    rules_per_shard_max: usize,
    modules: HashMap<String, CatalogItem>,
    manifest: String,
    functional_manifest: String,
    functional_sections: HashMap<String, usize>,
    shards: Vec<ShardItem>,
}

#[derive(Clone)]
struct SourceEntry {
    manifest_path: PathBuf,
    source: String,
    policy: String,
    kind: String,
}

impl SourceEntry {
    fn is_remote(&self) -> bool {
        self.source.starts_with("http://") || self.source.starts_with("https://")
    }

    fn resolved_path(&self, root_dir: &Path) -> PathBuf {
        if self.is_remote() {
            return PathBuf::from(&self.source);
        }
        let decoded = percent_encoding::percent_decode_str(&self.source)
            .decode_utf8_lossy()
            .to_string();
        if decoded.starts_with('/') {
            root_dir.join(decoded.trim_start_matches('/'))
        } else {
            self.manifest_path.parent().unwrap().join(&decoded)
        }
    }

    fn display_name(&self, root_dir: &Path) -> String {
        if self.is_remote() {
            self.source
                .split('/')
                .last()
                .unwrap_or(&self.source)
                .to_string()
        } else {
            match self.resolved_path(root_dir).strip_prefix(root_dir) {
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => self
                    .resolved_path(root_dir)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| self.source.clone()),
            }
        }
    }
}

pub struct AdBlockManager {
    root_dir: PathBuf,
    safety: crate::promax::safety::SafetyPolicy,
    quarantine: Vec<crate::promax::safety::QuarantineRecord>,
    rules: HashMap<String, HashMap<String, HashSet<String>>>, // policy -> category -> rules
    seen_rules: HashMap<String, HashSet<String>>,             // policy -> rules
    seen_suffixes: HashMap<String, HashSet<String>>,          // policy -> suffixes
    sections: HashMap<String, Vec<String>>,                   // section_name -> lines
    section_seen: HashMap<String, HashSet<String>>,           // section_name -> seen keys
    functional_stats: HashMap<String, usize>,
    mitm_hosts: HashSet<String>,
    mitm_provenance: HashMap<String, (String, usize)>,
    hashes: HashMap<String, String>,
    split_artifacts: Vec<crate::promax::artifact::GeneratedArtifact>,
    current_source: String,
    current_line: usize,
}

impl AdBlockManager {
    pub fn new(root_dir: PathBuf) -> Self {
        let mut rules = HashMap::new();
        let mut seen_rules = HashMap::new();
        let mut seen_suffixes = HashMap::new();

        for policy in &["REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT"] {
            let mut cats = HashMap::new();
            for cat in CATEGORIES {
                cats.insert(cat.to_string(), HashSet::new());
            }
            rules.insert(policy.to_string(), cats);
            seen_rules.insert(policy.to_string(), HashSet::new());
            seen_suffixes.insert(policy.to_string(), HashSet::new());
        }

        let mut sections = HashMap::new();
        let mut section_seen = HashMap::new();
        for name in &[
            "URL Rewrite",
            "Map Local",
            "Script",
            "Body Rewrite",
            "Header Rewrite",
        ] {
            sections.insert(name.to_string(), Vec::new());
            section_seen.insert(name.to_string(), HashSet::new());
        }

        AdBlockManager {
            root_dir,
            safety: crate::promax::safety::SafetyPolicy::default(),
            quarantine: Vec::new(),
            rules,
            seen_rules,
            seen_suffixes,
            sections,
            section_seen,
            functional_stats: HashMap::new(),
            mitm_hosts: HashSet::new(),
            mitm_provenance: HashMap::new(),
            hashes: HashMap::new(),
            split_artifacts: Vec::new(),
            current_source: "unknown".to_string(),
            current_line: 0,
        }
    }

    pub fn load_whitelist(&mut self) -> Result<(), String> {
        let path = self
            .root_dir
            .join("rulesets/Sources/adblock_whitelist.list");
        let content = fs::read_to_string(&path).map_err(|error| {
            format!(
                "required whitelist {} is unreadable: {error}",
                path.display()
            )
        })?;
        let (safety, stats) =
            crate::promax::safety::SafetyPolicy::from_lines_with_stats(content.lines());
        if !stats.is_publish_ready() {
            return Err(format!(
                "whitelist safety floor failed: {} domain patterns, {} IP networks, {} invalid entries",
                stats.domain_patterns, stats.ip_networks, stats.invalid_entries
            ));
        }
        self.safety = safety;
        println!(
            "\x1b[0;32m[INFO]\x1b[0m Loaded {} protected whitelist patterns ({} domain, {} IP)",
            stats.domain_patterns + stats.ip_networks,
            stats.domain_patterns,
            stats.ip_networks
        );
        Ok(())
    }

    pub fn load_hashes(&mut self) {
        let path = self.root_dir.join(".cache/adblock_hashes.list");
        if !path.exists() {
            return;
        }
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                let parts: Vec<&str> = line.trim().split('|').collect();
                if parts.len() == 2 {
                    self.hashes
                        .insert(parts[0].to_string(), parts[1].to_string());
                }
            }
        }
    }

    pub fn save_hashes(&self, hashes: &HashMap<String, String>) {
        let mut keys: Vec<&String> = hashes.keys().collect();
        keys.sort();
        let mut content = String::new();
        for k in keys {
            content.push_str(&format!("{}|{}\n", k, hashes.get(k).unwrap()));
        }
        let path = self.root_dir.join(".cache/adblock_hashes.list");
        let _ = crate::safe_write_file_internal(&path, &content, false);
    }

    fn load_source_entries(&self) -> Vec<SourceEntry> {
        let mut entries = Vec::new();
        let path = self
            .root_dir
            .join("rulesets/Sources/Links/AdBlock_sources.list");
        if !path.exists() {
            println!("\x1b[0;33m[WARN]\x1b[0m Sources file not found: {:?}", path);
            return entries;
        }
        let mut seen = HashSet::new();
        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<String> = line.split('|').map(|s| s.trim().to_string()).collect();
                let source = parts[0].clone();
                let policy = if parts.len() > 1 && !parts[1].is_empty() {
                    parts[1].to_uppercase()
                } else {
                    "REJECT".to_string()
                };
                let kind = if parts.len() > 2 && !parts[2].is_empty() {
                    parts[2].to_lowercase()
                } else {
                    "auto".to_string()
                };

                let valid_policies = [
                    "REJECT",
                    "REJECT-DROP",
                    "REJECT-NO-DROP",
                    "REJECT-TINYGIF",
                    "REJECT-IMG",
                    "DIRECT",
                    "PROXY",
                ];
                if !valid_policies.contains(&policy.as_str()) {
                    println!(
                        "\x1b[0;33m[WARN]\x1b[0m Skipping invalid policy in AdBlock manifest: {}",
                        line
                    );
                    continue;
                }
                if kind != "auto" && kind != "domainset" {
                    println!(
                        "\x1b[0;33m[WARN]\x1b[0m Skipping invalid source kind in AdBlock manifest: {}",
                        line
                    );
                    continue;
                }

                let entry = SourceEntry {
                    manifest_path: path.clone(),
                    source,
                    policy,
                    kind,
                };
                let key = format!("{}|{}|{}", entry.source, entry.policy, entry.kind);
                if seen.insert(key) {
                    entries.push(entry);
                }
            }
        }
        println!(
            "\x1b[0;32m[INFO]\x1b[0m Loaded {} canonical AdBlock sources",
            entries.len()
        );
        entries
    }

    fn download_remote_contents(
        &self,
        source_entries: &[SourceEntry],
    ) -> Result<HashMap<String, String>, Vec<crate::promax::source::DownloadFailure>> {
        let mut urls: Vec<String> = source_entries
            .iter()
            .filter(|entry| entry.is_remote())
            .map(|entry| entry.source.clone())
            .collect();
        let (_, functional_urls) = self.load_functional_source_paths();
        urls.extend(functional_urls);

        println!(
            "\x1b[0;32m[INFO]\x1b[0m Rust downloader resolving {} remote PROMAX source(s)",
            urls.len()
        );
        let downloader = crate::promax::source::Downloader::new(
            crate::promax::source::DownloadConfig::default(),
        );
        let batch = downloader.download_many(urls);
        if batch.failures.is_empty() {
            Ok(batch.contents)
        } else {
            Err(batch.failures)
        }
    }

    fn determine_category(&self, source: &str) -> &'static str {
        let lowered = source.to_lowercase();
        if lowered.contains("/source/local") || lowered.contains("localmodules") {
            return "Local";
        }
        if lowered.contains("ultimate.list") || lowered.contains("/ultimate") {
            return "ThreatIntel_Ultimate";
        }
        if lowered.contains("tif.medium") || lowered.contains("/tif.") {
            return "ThreatIntel_TIF";
        }
        if lowered.contains("hagezi") || lowered.contains("reject_phishing") {
            return "ThreatIntel";
        }
        if lowered.contains("anti-ad") {
            return "AntiAD";
        }
        for kw in &["privacy", "tracking", "tracking-protection"] {
            if lowered.contains(kw) {
                return "Privacy";
            }
        }
        for kw in &[
            "hijacking",
            "phishing",
            "threat",
            "banprogram",
            "reject.list",
            "reject-url",
            "reject_extra",
            "blockhttpdns",
            "httpdns",
        ] {
            if lowered.contains(kw) {
                return "Security";
            }
        }
        for kw in &["advertising", "ads", "banad", "adblock", "adg.list"] {
            if lowered.contains(kw) {
                return "Advertising";
            }
        }
        "Other"
    }

    fn category_from_filename(&self, filename: &str) -> &'static str {
        let stem = filename.replace(".list", "");
        if !stem.starts_with("AdBlock_") {
            return "Other";
        }
        let body = &stem[8..];

        let mut sorted_cats = CATEGORIES.to_vec();
        sorted_cats.sort_by_key(|c| std::cmp::Reverse(c.len()));

        for cat in sorted_cats {
            if body == cat || body.starts_with(&format!("{}_", cat)) {
                return cat;
            }
        }
        "Other"
    }

    fn resolve_module_ingest_mode(&self, path: &Path, from_manifest: bool) -> &'static str {
        if !path.is_file() {
            return "skip";
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if FUNCTIONAL_BLOCK_LOCAL_SOURCES.contains(&name.as_str()) {
            return "skip";
        }
        if ADBLOCK_FILENAME_ALLOWLIST.contains(&name.as_str()) {
            return "full";
        }
        let low = name.to_lowercase();

        if !from_manifest {
            let parent = path.parent().unwrap();
            let nexus_dir = self.root_dir.join("modules/surge/amplify_nexus");
            if parent == nexus_dir {
                return "skip";
            }
        }

        if !from_manifest {
            for tok in FUNCTIONAL_BLOCK_NAME_TOKENS {
                if low.contains(tok) {
                    return "skip";
                }
            }
        }

        if let Ok(text) = fs::read_to_string(path) {
            let mode = crate::smart_cleanup::module_ingest_mode_pub(path.to_str().unwrap(), &text);
            if mode != "skip" {
                if mode == "full" {
                    return "full";
                } else if mode == "split" {
                    return "split";
                }
            }
        }

        let parent = path.parent().unwrap();
        let local_dir = self.root_dir.join("modules/source/local");
        if parent == local_dir {
            if name.ends_with(".sgmodule") || name.ends_with(".module") {
                return "full";
            }
            return "skip";
        }

        for tok in ADBLOCK_MODULE_NAME_TOKENS {
            if low.contains(tok) {
                return "full";
            }
        }
        "skip"
    }

    fn is_script_or_rewrite_line(&self, line: &str) -> bool {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            return false;
        }
        let lowered = stripped.to_lowercase();
        for token in &[
            "script-path=",
            "requires-body=",
            "type=http-request",
            "type=http-response",
            "binary-body-mode=",
            "max-size=",
            "data-type=",
            "status-code=",
        ] {
            if lowered.contains(token) {
                return true;
            }
        }
        if stripped.starts_with('^') && (lowered.contains(" reject") || lowered.contains(" 302")) {
            return true;
        }
        if lowered.contains(" _ reject") || lowered.contains(" - reject") {
            return true;
        }
        false
    }

    fn infer_non_rule_section(&self, line: &str) -> Option<&'static str> {
        if line.is_empty() || line.starts_with("#!") {
            return None;
        }
        let stripped = line.trim();
        let lowered = stripped.to_lowercase();

        if stripped.starts_with("hostname") {
            return Some("MITM");
        }
        if lowered.contains("data-type=") || lowered.contains("status-code=") {
            return Some("Map Local");
        }
        if lowered.contains("script-path=")
            || lowered.contains("requires-body=")
            || lowered.contains("type=http-")
        {
            return Some("Script");
        }
        for prefix in &[
            "http-response",
            "http-request",
            "http-response-jq",
            "http-request-jq",
        ] {
            if lowered.starts_with(prefix) {
                return Some("Body Rewrite");
            }
        }
        if stripped.starts_with('^')
            || lowered.contains(" - reject")
            || lowered.contains(" _ reject")
            || lowered.ends_with(" 302")
        {
            return Some("URL Rewrite");
        }
        None
    }

    fn render_policy_rule(&self, normalized_rule: &str) -> String {
        normalized_rule.to_string()
    }

    fn add_rule_line(&mut self, line: &str, default_policy: &str, category: &str) {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            return;
        }
        if self.is_script_or_rewrite_line(stripped) {
            return;
        }
        if stripped.starts_with("AND,")
            || stripped.starts_with("OR,")
            || stripped.starts_with("NOT,")
        {
            self.quarantine
                .push(crate::promax::safety::QuarantineRecord {
                    source: self.current_source.clone(),
                    line: self.current_line,
                    candidate: stripped.to_string(),
                    reason: "logical-rule-not-flattened".to_string(),
                    risk: crate::promax::safety::RiskClass::Invalid,
                });
            return;
        }
        let candidates = vec![stripped.to_string()];

        for candidate in candidates {
            let cleaned = crate::strip_inline_comment(&candidate);
            let parse_candidate = if cleaned.contains(',') {
                Some(cleaned.clone())
            } else {
                crate::promax::functional::normalize_domain_source_line(&cleaned)
            };
            let Some(parse_candidate) = parse_candidate else {
                continue;
            };
            let parsed = match crate::promax::rule::SurgeRule::parse(&parse_candidate) {
                Ok(rule) => rule,
                Err(parse_error) => {
                    self.quarantine
                        .push(crate::promax::safety::QuarantineRecord {
                            source: self.current_source.clone(),
                            line: self.current_line,
                            candidate: candidate.clone(),
                            reason: parse_error.to_string(),
                            risk: crate::promax::safety::RiskClass::Invalid,
                        });
                    continue;
                }
            };
            let policy = parsed
                .policy
                .as_deref()
                .unwrap_or(default_policy)
                .to_ascii_uppercase();
            let policy = match policy.as_str() {
                "PROXY" => {
                    self.quarantine
                        .push(crate::promax::safety::QuarantineRecord {
                            source: self.current_source.clone(),
                            line: self.current_line,
                            candidate: candidate.clone(),
                            reason: "unsupported-proxy-policy".to_string(),
                            risk: crate::promax::safety::RiskClass::Invalid,
                        });
                    continue;
                }
                "REJECT-TINYGIF" | "REJECT-IMG" => "REJECT".to_string(),
                _ => policy,
            };

            if let crate::promax::safety::SafetyDecision::Quarantine(reason) =
                self.safety.decision(&parsed)
            {
                self.quarantine
                    .push(crate::promax::safety::QuarantineRecord {
                        source: self.current_source.clone(),
                        line: self.current_line,
                        candidate: candidate.clone(),
                        reason: reason.to_string(),
                        risk: crate::promax::safety::RiskClass::ProtectedService,
                    });
                continue;
            }

            let normalized = parsed.render_external();
            let rule_payload = parsed.payload.clone();

            let rendered = self.render_policy_rule(&normalized);

            // Deduplication
            let seen_bucket = self.seen_rules.get_mut(&policy).unwrap();
            if seen_bucket.contains(&rendered) {
                continue;
            }

            // Suffix subsumption
            let rtype = parsed.kind.as_str();
            let rval = rule_payload.to_lowercase();
            let policy_suffixes = self.seen_suffixes.get_mut(&policy).unwrap();
            if rtype == "DOMAIN" || rtype == "DOMAIN-SUFFIX" {
                if rtype == "DOMAIN" {
                    let parts: Vec<&str> = rval.split('.').collect();
                    let mut matched = false;
                    for i in 0..(parts.len() - 1) {
                        let suffix = parts[i..].join(".");
                        if policy_suffixes.contains(&suffix) {
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        continue;
                    }
                }
                if rtype == "DOMAIN-SUFFIX" {
                    policy_suffixes.insert(rval);
                }
            }

            let cat_bucket = self
                .rules
                .get_mut(&policy)
                .unwrap()
                .get_mut(category)
                .unwrap();
            cat_bucket.insert(rendered.clone());
            seen_bucket.insert(rendered);
        }
    }

    fn add_section_line(&mut self, section_name: &str, line: &str) {
        if !self.sections.contains_key(section_name) {
            return;
        }
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            return;
        }

        if let crate::promax::safety::SafetyDecision::Quarantine(reason) =
            self.safety.functional_decision(stripped)
        {
            let risk = if reason == "protected-functional-intersection" {
                crate::promax::safety::RiskClass::ProtectedService
            } else {
                crate::promax::safety::RiskClass::BroadMedia
            };
            self.quarantine
                .push(crate::promax::safety::QuarantineRecord {
                    source: self.current_source.clone(),
                    line: self.current_line,
                    candidate: stripped.to_string(),
                    reason: reason.to_string(),
                    risk,
                });
            return;
        }

        let key = crate::smart_cleanup::dedupe_key_pub(section_name, stripped);
        if key.is_empty() {
            return;
        }
        let seen = self.section_seen.get_mut(section_name).unwrap();
        if seen.insert(key) {
            self.sections
                .get_mut(section_name)
                .unwrap()
                .push(stripped.to_string());
        }
    }

    fn add_mitm_host(&mut self, hostname: &str) {
        let hostname = hostname.trim();
        if hostname.is_empty() {
            return;
        }
        if self.safety.hostname_is_protected(hostname) {
            self.quarantine
                .push(crate::promax::safety::QuarantineRecord {
                    source: self.current_source.clone(),
                    line: self.current_line,
                    candidate: hostname.to_string(),
                    reason: "protected-mitm-host".to_string(),
                    risk: crate::promax::safety::RiskClass::ProtectedService,
                });
            return;
        }
        self.mitm_provenance
            .entry(hostname.to_string())
            .or_insert_with(|| (self.current_source.clone(), self.current_line));
        self.mitm_hosts.insert(hostname.to_string());
    }

    fn retain_referenced_mitm_hosts(&mut self) {
        let functional_lines: Vec<String> = self
            .sections
            .values()
            .flat_map(|lines| lines.iter().cloned())
            .collect();
        let removed: Vec<String> = self
            .mitm_hosts
            .iter()
            .filter(|hostname| {
                !self
                    .safety
                    .hostname_is_referenced(hostname, &functional_lines)
            })
            .cloned()
            .collect();
        for hostname in removed {
            self.mitm_hosts.remove(&hostname);
            let (source, line) = self
                .mitm_provenance
                .remove(&hostname)
                .unwrap_or_else(|| ("MITM".to_string(), 0));
            self.quarantine
                .push(crate::promax::safety::QuarantineRecord {
                    source,
                    line,
                    candidate: hostname,
                    reason: "unreferenced-mitm-host".to_string(),
                    risk: crate::promax::safety::RiskClass::BroadMedia,
                });
        }
    }

    fn extract_from_text(
        &mut self,
        text: &str,
        default_policy: &str,
        category: &str,
        include_sections: bool,
        rules_only: bool,
        source: &str,
    ) {
        let lines: Vec<&str> = text.lines().collect();
        let mut current_section = String::new();
        let mut in_rule_section = false;
        let mut seen_real_header = false;

        let rule_sections: HashSet<&str> = ["Rule", "Adblock Plus"].iter().cloned().collect();

        self.current_source = source.to_string();
        for (index, line) in lines.into_iter().enumerate() {
            self.current_line = index + 1;
            let stripped = line.trim();
            if stripped.is_empty() {
                continue;
            }

            if stripped.starts_with('[') && stripped.ends_with(']') && !stripped.starts_with('#') {
                current_section = stripped[1..stripped.len() - 1].to_string();
                in_rule_section = rule_sections.contains(current_section.as_str());
                seen_real_header = true;
                continue;
            }

            if !seen_real_header {
                if rules_only {
                    continue;
                }
                if self.is_script_or_rewrite_line(stripped) {
                    continue;
                }
                if crate::promax::rule::SurgeRule::parse(stripped).is_ok()
                    || crate::promax::functional::normalize_domain_source_line(stripped).is_some()
                {
                    self.add_rule_line(stripped, default_policy, category);
                    continue;
                }
                if include_sections {
                    if let Some(inferred) = self.infer_non_rule_section(stripped) {
                        if inferred == "MITM" && stripped.starts_with("hostname") {
                            let hosts = stripped
                                .splitn(2, '=')
                                .nth(1)
                                .unwrap_or("")
                                .replace("%APPEND%", "")
                                .replace("%INSERT%", "");
                            for h in hosts.split(',') {
                                let h = h.trim();
                                if !h.is_empty() {
                                    self.add_mitm_host(h);
                                }
                            }
                        } else {
                            self.add_section_line(inferred, stripped);
                        }
                    }
                }
                continue;
            }

            if in_rule_section {
                self.add_rule_line(stripped, default_policy, category);
            } else if include_sections && !stripped.starts_with('#') {
                if self.sections.contains_key(&current_section) {
                    self.add_section_line(&current_section, stripped);
                } else if current_section == "MITM" && stripped.starts_with("hostname") {
                    let hosts = stripped
                        .splitn(2, '=')
                        .nth(1)
                        .unwrap_or("")
                        .replace("%APPEND%", "")
                        .replace("%INSERT%", "");
                    for h in hosts.split(',') {
                        let h = h.trim();
                        if !h.is_empty() {
                            self.add_mitm_host(h);
                        }
                    }
                }
            }
        }
    }

    fn extract_from_file(
        &mut self,
        path: &Path,
        default_policy: &str,
        category: &str,
        include_sections: bool,
        rules_only: bool,
    ) {
        if !path.exists() {
            return;
        }
        if let Ok(text) = fs::read_to_string(path) {
            let source = path
                .strip_prefix(&self.root_dir)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            self.extract_from_text(
                &text,
                default_policy,
                category,
                include_sections,
                rules_only,
                &source,
            );
        }
    }

    fn process_source_entries(
        &mut self,
        source_entries: &[SourceEntry],
        remote_contents: &HashMap<String, String>,
    ) -> Vec<String> {
        let mut failures = Vec::new();
        for entry in source_entries {
            let category = self.determine_category(&entry.source).to_string();
            let is_module = entry.source.ends_with(".sgmodule")
                || entry.source.ends_with(".module")
                || entry
                    .resolved_path(&self.root_dir)
                    .to_string_lossy()
                    .ends_with(".sgmodule")
                || entry
                    .resolved_path(&self.root_dir)
                    .to_string_lossy()
                    .ends_with(".module");

            let local_text;
            let mod_text = if entry.is_remote() {
                remote_contents.get(&entry.source).map(String::as_str)
            } else {
                local_text = fs::read_to_string(entry.resolved_path(&self.root_dir)).ok();
                local_text.as_deref()
            };

            let mod_text = match mod_text {
                Some(text) => text,
                None => {
                    println!(
                        "\x1b[0;33m[WARN]\x1b[0m   ⚠ Failed: {}",
                        entry.display_name(&self.root_dir)
                    );
                    failures.push(entry.display_name(&self.root_dir));
                    continue;
                }
            };

            if is_module {
                let has_rule_section = {
                    let mut has = false;
                    for line in mod_text.lines() {
                        let stripped = line.trim();
                        if stripped.starts_with('[')
                            && stripped.ends_with(']')
                            && !stripped.starts_with('#')
                        {
                            let name = &stripped[1..stripped.len() - 1];
                            if name == "Rule" || name == "Adblock Plus" {
                                has = true;
                                break;
                            }
                        }
                    }
                    has
                };
                if !has_rule_section {
                    println!(
                        "\x1b[0;32m[INFO]\x1b[0m   ⊘ Skip (no [Rule] section, script/rewrite-only): {}",
                        entry.display_name(&self.root_dir)
                    );
                    continue;
                }
                println!(
                    "\x1b[0;32m[INFO]\x1b[0m Fetching source: {} (Category: {})",
                    entry.display_name(&self.root_dir),
                    category
                );
                self.extract_from_text(
                    mod_text,
                    &entry.policy,
                    &category,
                    false,
                    true,
                    &entry.display_name(&self.root_dir),
                );
                println!(
                    "\x1b[0;32m[SUCCESS]\x1b[0m   ✓ {}",
                    entry.display_name(&self.root_dir)
                );
            } else {
                println!(
                    "\x1b[0;32m[INFO]\x1b[0m Fetching source: {} (Category: {})",
                    entry.display_name(&self.root_dir),
                    category
                );
                self.extract_from_text(
                    mod_text,
                    &entry.policy,
                    &category,
                    false,
                    false,
                    &entry.display_name(&self.root_dir),
                );
                println!(
                    "\x1b[0;32m[SUCCESS]\x1b[0m   ✓ {}",
                    entry.display_name(&self.root_dir)
                );
            }
        }
        failures
    }

    fn filter_rules(&self, rules: &HashSet<String>) -> Vec<String> {
        let mut filtered = Vec::new();
        for rule in rules {
            if let Ok(parsed) = crate::promax::rule::SurgeRule::parse(rule) {
                if matches!(
                    self.safety.decision(&parsed),
                    crate::promax::safety::SafetyDecision::Quarantine(_)
                ) {
                    continue;
                }
            }
            filtered.push(rule.clone());
        }
        filtered.sort();
        filtered
    }

    fn generate_ruleset(&self) -> (Vec<String>, Vec<crate::promax::artifact::GeneratedArtifact>) {
        let mut generated_files = Vec::new();
        let mut artifacts = Vec::new();
        let adblock_dir = self.root_dir.join("rulesets/AdBlock");
        if !adblock_dir.exists() {
            let _ = fs::create_dir_all(&adblock_dir);
        }

        println!(
            "\x1b[0;32m[INFO]\x1b[0m Generating split rulesets in rulesets/AdBlock (≤{} rules/shard)",
            RULES_PER_SHARD
        );

        for category in CATEGORIES {
            let empty_set = HashSet::new();
            let rules_in_cat = self
                .rules
                .get("REJECT")
                .unwrap()
                .get(*category)
                .unwrap_or(&empty_set);
            let rules = self.filter_rules(rules_in_cat);

            if rules.is_empty() {
                println!(
                    "\x1b[0;32m[INFO]\x1b[0m   ⊘ Skip shard for {}: no unique rules",
                    category
                );
                continue;
            }

            let meta = CATEGORY_META.get(category).unwrap();
            let chunks: Vec<&[String]> = rules.chunks(RULES_PER_SHARD).collect();

            for (idx, chunk) in chunks.iter().enumerate() {
                let filename = if chunks.len() == 1 {
                    format!("AdBlock_{}.list", category)
                } else {
                    format!("AdBlock_{}_{:02}.list", category, idx + 1)
                };
                let shard_label = if chunks.len() == 1 {
                    "1/1".to_string()
                } else {
                    format!("{}/{}", idx + 1, chunks.len())
                };

                let filepath = adblock_dir.join(&filename);
                let mut content = Vec::new();
                content.push(format!("# Ruleset: AdBlock · {}", meta.label_zh));
                content.push(format!("# Purpose: {}", meta.desc));
                content.push(format!("# Category-Key: {}", category));
                content.push(format!("# Shard: {}", shard_label));
                content.push(format!("# Total Rules: {}", chunk.len()));

                for r in *chunk {
                    content.push(r.clone());
                }

                let mut out_str = content.join("\n");
                out_str.push('\n');
                let rel_path = format!("rulesets/AdBlock/{}", filename);
                artifacts.push(crate::promax::artifact::GeneratedArtifact {
                    target: filepath,
                    content: out_str,
                    kind: crate::promax::artifact::ArtifactKind::ExternalRuleset,
                });
                generated_files.push(rel_path);
                println!(
                    "\x1b[0;32m[SUCCESS]\x1b[0m   ✓ {} ({} rules, {})",
                    filename,
                    chunk.len(),
                    meta.label_zh
                );
            }
        }

        // Legacy AdBlock.list
        let legacy_path = self.root_dir.join("rulesets/RULE-SET/AdBlock.list");
        let mut legacy_content = Vec::new();
        legacy_content.push("# Ruleset: AdBlock (LEGACY / REDIRECT)".to_string());
        legacy_content
            .push("# This file is now split into multiple files in rulesets/AdBlock/".to_string());
        legacy_content
            .push("# Please update your module to use the new split rulesets.".to_string());

        let mut all_reject = Vec::new();
        for cat in CATEGORIES {
            let empty_set = HashSet::new();
            let rules_in_cat = self
                .rules
                .get("REJECT")
                .unwrap()
                .get(*cat)
                .unwrap_or(&empty_set);
            all_reject.extend(self.filter_rules(rules_in_cat));
        }
        let limit = std::cmp::min(1000, all_reject.len());
        for r in &all_reject[..limit] {
            legacy_content.push(r.clone());
        }

        let mut legacy_str = legacy_content.join("\n");
        legacy_str.push('\n');
        artifacts.push(crate::promax::artifact::GeneratedArtifact {
            target: legacy_path,
            content: legacy_str,
            kind: crate::promax::artifact::ArtifactKind::ExternalRuleset,
        });

        (generated_files, artifacts)
    }

    fn prune_stale_rulesets(&self, generated_files: &[String]) {
        let adblock_dir = self.root_dir.join("rulesets/AdBlock");
        if !adblock_dir.is_dir() {
            return;
        }
        let mut keep = HashSet::new();
        for f in generated_files {
            keep.insert(f.split('/').last().unwrap().to_string());
        }
        keep.insert("HTTPDNS_Hijack.list".to_string());
        keep.insert("reject-drop.list".to_string());
        keep.insert("reject-no-drop.list".to_string());
        keep.insert("AdBlock.list".to_string());

        let mut removed = Vec::new();
        if let Ok(entries) = fs::read_dir(adblock_dir) {
            for entry in entries.map_while(Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".list") && !keep.contains(&name) {
                    let _ = fs::remove_file(entry.path());
                    removed.push(name.clone());
                    let srs_name = format!("{}_Singbox.srs", name.replace(".list", ""));
                    let srs_path = self.root_dir.join("rulesets/SingBox").join(&srs_name);
                    if srs_path.exists() {
                        let _ = fs::remove_file(srs_path);
                    }
                }
            }
        }
        if !removed.is_empty() {
            println!(
                "\x1b[0;32m[INFO]\x1b[0m Pruned {} stale AdBlock ruleset(s): {}",
                removed.len(),
                removed.join(", ")
            );
        }
    }

    fn integrate_functional_sections(
        &mut self,
        remote_contents: &HashMap<String, String>,
    ) -> HashMap<String, usize> {
        println!(
            "\x1b[0;32m[INFO]\x1b[0m ============================================================"
        );
        println!(
            "\x1b[0;32m[INFO]\x1b[0m 🚀 Integrating Functional Sections (Script / Rewrite / MITM)"
        );
        println!(
            "\x1b[0;32m[INFO]\x1b[0m ============================================================"
        );

        let mut counts_before = HashMap::new();
        for name in &[
            "URL Rewrite",
            "Map Local",
            "Script",
            "Body Rewrite",
            "Header Rewrite",
        ] {
            counts_before.insert(name.to_string(), self.sections.get(*name).unwrap().len());
        }
        let mitm_before = self.mitm_hosts.len();

        let (local_paths, remote_urls) = self.load_functional_source_paths();

        for u in remote_urls {
            if let Some(content) = remote_contents.get(&u) {
                let parts: Vec<&str> = u.split('/').collect();
                println!(
                    "\x1b[0;32m[INFO]\x1b[0m   + Functional remote: {}",
                    parts.last().unwrap_or(&u.as_str())
                );
                self.extract_from_text(content, "REJECT", "Local", true, false, &u);
            } else {
                println!(
                    "\x1b[0;33m[WARN]\x1b[0m   ⚠ Functional remote fetch failed: {}",
                    u
                );
            }
        }

        for (path, ingest_mode) in local_paths {
            let tag = if ingest_mode == "split" {
                "split"
            } else {
                "full"
            };
            let rel = path
                .strip_prefix(&self.root_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            println!("\x1b[0;32m[INFO]\x1b[0m   + Functional ({}): {}", tag, rel);

            if let Ok(content) = fs::read_to_string(&path) {
                self.extract_from_text(&content, "REJECT", "Local", true, false, &rel);
            }
            if ingest_mode == "split" {
                self.write_promax_split_artifact(&path);
            }
        }

        self.retain_referenced_mitm_hosts();

        let mut added = HashMap::new();
        let mut summary_parts = Vec::new();
        for name in &[
            "URL Rewrite",
            "Map Local",
            "Script",
            "Body Rewrite",
            "Header Rewrite",
        ] {
            let before = *counts_before.get(*name).unwrap();
            let after = self.sections.get(*name).unwrap().len();
            let diff = after - before;
            if diff > 0 {
                added.insert(name.to_string(), diff);
                summary_parts.push(format!("{}={}", name, diff));
            }
        }
        let diff_mitm = self.mitm_hosts.len() - mitm_before;
        if diff_mitm > 0 {
            added.insert("MITM".to_string(), diff_mitm);
            summary_parts.push(format!("MITM={}", diff_mitm));
        }

        let summary = if summary_parts.is_empty() {
            "no new lines".to_string()
        } else {
            summary_parts.join(", ")
        };
        println!(
            "\x1b[0;32m[SUCCESS]\x1b[0m Functional sections merged: {}",
            summary
        );
        added
    }

    fn write_promax_split_artifact(&mut self, source_path: &Path) {
        if let Ok(text) = fs::read_to_string(source_path) {
            let (ad_sections, stats) = crate::smart_cleanup::split_module_sections_pub(
                source_path.to_str().unwrap(),
                &text,
            );
            let kept = *stats.get("kept_lines").unwrap_or(&0);
            let total = *stats.get("total_lines").unwrap_or(&0);
            if kept == 0 {
                return;
            }
            let out_dir = self.root_dir.join("modules/source/build/promax_splits");
            let _ = fs::create_dir_all(&out_dir);

            let stem = source_path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let safe_stem = Regex::new(r"[^\w\u4e00-\u9fff.-]+")
                .unwrap()
                .replace_all(&stem, "_")
                .to_string();
            let out_path = out_dir.join(format!("{}.ad-extract.sgmodule", safe_stem));

            let note = format!(
                "从混合模块拆出的去广告行 {}/{}；解锁/增强仍留在原模块",
                kept, total
            );

            let formatted = crate::smart_cleanup::format_split_module_pub(
                source_path.to_str().unwrap(),
                &ad_sections,
                &note,
            );
            let rel = out_path
                .strip_prefix(&self.root_dir)
                .unwrap_or(&out_path)
                .to_string_lossy()
                .to_string();
            self.split_artifacts
                .push(crate::promax::artifact::GeneratedArtifact {
                    target: out_path,
                    content: formatted,
                    kind: crate::promax::artifact::ArtifactKind::Surge,
                });
            println!(
                "\x1b[0;32m[INFO]\x1b[0m   ↳ split artifact: {} ({}/{} lines)",
                rel, kept, total
            );
        }
    }

    fn load_functional_source_paths(&self) -> (Vec<(PathBuf, &'static str)>, Vec<String>) {
        let mut paths = Vec::new();
        let mut remote_urls = Vec::new();
        let mut seen = HashSet::new();

        let func_sources_file = self
            .root_dir
            .join("rulesets/Sources/Links/AdBlock_functional_sources.list");
        if func_sources_file.exists() {
            if let Ok(file) = File::open(&func_sources_file) {
                let reader = BufReader::new(file);
                for line in reader.lines().map_while(Result::ok) {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if line.starts_with("http://") || line.starts_with("https://") {
                        remote_urls.push(line.to_string());
                        continue;
                    }
                    let resolved = func_sources_file.parent().unwrap().join(line);
                    let norm = resolved.canonicalize().unwrap_or(resolved);
                    if seen.insert(norm.clone()) && norm.is_file() {
                        let mode = self.resolve_module_ingest_mode(&norm, true);
                        if mode == "skip" {
                            let rel = norm
                                .strip_prefix(&self.root_dir)
                                .unwrap_or(&norm)
                                .to_string_lossy()
                                .to_string();
                            println!(
                                "\x1b[0;33m[WARN]\x1b[0m   ⊘ Skipped (no ad / unlock-only): {}",
                                rel
                            );
                            continue;
                        }
                        paths.push((norm, mode));
                    }
                }
            }
        }

        // Local local modules dir
        let local_dir = self.root_dir.join("modules/source/local");
        if local_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(local_dir) {
                let mut files: Vec<PathBuf> =
                    entries.map_while(Result::ok).map(|e| e.path()).collect();
                files.sort();
                for f in files {
                    let name = f.file_name().unwrap().to_string_lossy().to_string();
                    if name.ends_with(".sgmodule") || name.ends_with(".module") {
                        let norm = f.canonicalize().unwrap_or(f);
                        if seen.insert(norm.clone()) {
                            let mode = self.resolve_module_ingest_mode(&norm, false);
                            if mode != "skip" {
                                paths.push((norm, mode));
                            }
                        }
                    }
                }
            }
        }

        // LocalModules dir
        let local_modules_dir = self.root_dir.join("rulesets/Sources/LocalModules");
        if local_modules_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(local_modules_dir) {
                let mut files: Vec<PathBuf> =
                    entries.map_while(Result::ok).map(|e| e.path()).collect();
                files.sort();
                let protected = ["script_hub.surge.sgmodule"];
                for f in files {
                    let name = f.file_name().unwrap().to_string_lossy().to_string();
                    if protected.contains(&name.as_str()) {
                        continue;
                    }
                    if name.ends_with(".sgmodule") || name.ends_with(".module") {
                        let norm = f.canonicalize().unwrap_or(f);
                        if seen.insert(norm.clone()) {
                            let mode = self.resolve_module_ingest_mode(&norm, false);
                            if mode != "skip" {
                                paths.push((norm, mode));
                            }
                        }
                    }
                }
            }
        }

        let split_n = paths.iter().filter(|(_, m)| *m == "split").count();
        println!(
            "\x1b[0;32m[INFO]\x1b[0m Resolved {} functional module(s) ({} mixed→line-split), {} remote URL(s)",
            paths.len(),
            split_n,
            remote_urls.len()
        );
        (paths, remote_urls)
    }

    fn generate_catalog_artifacts(
        &self,
        generated_rulesets: &[String],
        ruleset_artifacts: &[crate::promax::artifact::GeneratedArtifact],
    ) -> Vec<crate::promax::artifact::GeneratedArtifact> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let surge_promax_cdn = "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule";
        let surge_promax_github = "modules/surge/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule";
        let loon_promax_cdn = "modules/loon/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).plugin";
        let loon_promax_github = "modules/loon/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).plugin";

        let sr_promax_cdn = "modules/shadowrocket/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module";
        let sr_promax_github = "modules/shadowrocket/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module";

        let mut modules = HashMap::new();
        modules.insert(
            "surge_promax_cdn".to_string(),
            CatalogItem {
                path: surge_promax_cdn.to_string(),
                install_url: format!(
                    "{}{}",
                    CDN_BASE_URL,
                    percent_encoding::utf8_percent_encode(
                        surge_promax_cdn,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                ),
                url_source: "cdn".to_string(),
                note: "CDN 加速版（推荐，12-24h 缓存延迟）".to_string(),
            },
        );
        modules.insert(
            "surge_promax_github".to_string(),
            CatalogItem {
                path: surge_promax_github.to_string(),
                install_url: format!(
                    "{}{}",
                    GITHUB_RAW_URL,
                    percent_encoding::utf8_percent_encode(
                        surge_promax_github,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                ),
                url_source: "github".to_string(),
                note: "GitHub Raw 版（即时更新，内部规则集也用 GitHub Raw）".to_string(),
            },
        );
        modules.insert(
            "loon_promax_cdn".to_string(),
            CatalogItem {
                path: loon_promax_cdn.to_string(),
                install_url: format!(
                    "{}{}",
                    CDN_BASE_URL,
                    percent_encoding::utf8_percent_encode(
                        loon_promax_cdn,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                ),
                url_source: "cdn".to_string(),
                note: "Loon CDN 加速版（推荐）".to_string(),
            },
        );
        modules.insert(
            "loon_promax_github".to_string(),
            CatalogItem {
                path: loon_promax_github.to_string(),
                install_url: format!(
                    "{}{}",
                    GITHUB_RAW_URL,
                    percent_encoding::utf8_percent_encode(
                        loon_promax_github,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                ),
                url_source: "github".to_string(),
                note: "Loon GitHub Raw 版（即时更新）".to_string(),
            },
        );
        modules.insert(
            "shadowrocket_promax_cdn".to_string(),
            CatalogItem {
                path: sr_promax_cdn.to_string(),
                install_url: format!(
                    "{}{}",
                    CDN_BASE_URL,
                    percent_encoding::utf8_percent_encode(
                        sr_promax_cdn,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                ),
                url_source: "cdn".to_string(),
                note: "CDN 加速版（推荐，12-24h 缓存延迟）".to_string(),
            },
        );
        modules.insert(
            "shadowrocket_promax_github".to_string(),
            CatalogItem {
                path: sr_promax_github.to_string(),
                install_url: format!(
                    "{}{}",
                    GITHUB_RAW_URL,
                    percent_encoding::utf8_percent_encode(
                        sr_promax_github,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                ),
                url_source: "github".to_string(),
                note: "GitHub Raw 版（即时更新，内部规则集也用 GitHub Raw）".to_string(),
            },
        );

        let mut shards = Vec::new();
        for rs_path in generated_rulesets {
            let filename = rs_path.split('/').last().unwrap();
            let category = self.category_from_filename(filename);
            let meta = CATEGORY_META.get(category).unwrap();
            let match_idx = Regex::new(r"_(\d+)\.list$").unwrap();
            let shard_index = match_idx
                .captures(filename)
                .and_then(|caps| caps[1].parse::<usize>().ok())
                .unwrap_or(1);
            let full_path = self.root_dir.join(rs_path);
            let rule_count = ruleset_artifacts
                .iter()
                .find(|artifact| artifact.target == full_path)
                .map(|artifact| {
                    artifact
                        .content
                        .lines()
                        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
                        .count()
                })
                .unwrap_or(0);

            let encoded =
                percent_encoding::utf8_percent_encode(rs_path, percent_encoding::NON_ALPHANUMERIC)
                    .to_string();
            shards.push(ShardItem {
                file: rs_path.clone(),
                category: category.to_string(),
                label_zh: meta.label_zh.to_string(),
                purpose: meta.desc.to_string(),
                shard_index,
                rule_count,
                cdn_url: format!("{}{}", CDN_BASE_URL, encoded),
                github_url: format!("{}{}", GITHUB_RAW_URL, encoded),
            });
        }

        let mut functional_stats = HashMap::new();
        for (k, v) in &self.sections {
            functional_stats.insert(k.clone(), v.len());
        }
        if !self.mitm_hosts.is_empty() {
            functional_stats.insert("MITM".to_string(), self.mitm_hosts.len());
        }

        let catalog_data = CatalogJson {
            updated: now.clone(),
            rules_per_shard_max: RULES_PER_SHARD,
            modules,
            manifest: "rulesets/Sources/Links/AdBlock_sources.list".to_string(),
            functional_manifest: "rulesets/Sources/Links/AdBlock_functional_sources.list"
                .to_string(),
            functional_sections: functional_stats,
            shards,
        };

        let catalog_path = self.root_dir.join("rulesets/AdBlock/catalog.json");
        let json_str = serde_json::to_string_pretty(&catalog_data).unwrap();

        // Generate README.md
        let mut readme = Vec::new();
        readme.push("# AdBlock 分片规则集\n".to_string());
        readme.push(format!(
            "> 自动生成于 `{}` · 每片 ≤ {} 条 · 详见 [`catalog.json`](catalog.json)\n\n",
            now,
            format_commas(RULES_PER_SHARD)
        ));
        readme.push("## 用途分类\n\n| 分类 | 说明 |\n|------|------|\n".to_string());
        for key in CATEGORIES {
            let meta = CATEGORY_META.get(*key).unwrap();
            readme.push(format!(
                "| **{}** (`{}`) | {} |\n",
                meta.label_zh, key, meta.desc
            ));
        }
        readme.push("\n## 当前分片\n\n| 用途 | 文件 | 规则数 | CDN | GitHub |\n|------|------|--------|-----|--------|\n".to_string());
        for item in &catalog_data.shards {
            readme.push(format!(
                "| {} | `{}` | {} | [CDN]({}) | [GitHub]({}) |\n",
                item.label_zh,
                item.file,
                format_commas(item.rule_count),
                item.cdn_url,
                item.github_url
            ));
        }

        readme.push("\n## 应用内脚本层（PROMAX 内嵌）\n\n".to_string());
        for sec_name in &[
            "URL Rewrite",
            "Map Local",
            "Script",
            "Body Rewrite",
            "Header Rewrite",
        ] {
            let count = self.sections.get(*sec_name).unwrap().len();
            if count > 0 {
                readme.push(format!("| `{}` | {} |\n", sec_name, format_commas(count)));
            }
        }
        if !self.mitm_hosts.is_empty() {
            readme.push(format!(
                "| `MITM` (hostname) | {} |\n",
                format_commas(self.mitm_hosts.len())
            ));
        }

        readme.push("\n## 安装模块（RULE-SET + 去广告 Script/Rewrite；解锁/增强请用 Amplify Nexus 独立模块）\n\n".to_string());
        readme.push("`rulesets/Sources/LocalModules/*` 各 App 去广告逐文件并入 PROMAX，**勿单独安装**专项模块或已废弃的 narrow_pierce 副本。\n\n".to_string());
        readme.push("### 🖥️ 桌面完整版（Full）\n".to_string());
        readme.push(format!(
            "- **Surge PROMAX** CDN: {}\n",
            catalog_data
                .modules
                .get("surge_promax_cdn")
                .unwrap()
                .install_url
        ));
        readme.push(format!(
            "- **Surge PROMAX** GitHub: {}\n",
            catalog_data
                .modules
                .get("surge_promax_github")
                .unwrap()
                .install_url
        ));
        readme.push(format!(
            "- **Loon PROMAX** CDN: {}\n",
            catalog_data
                .modules
                .get("loon_promax_cdn")
                .unwrap()
                .install_url
        ));
        readme.push(format!(
            "- **Loon PROMAX** GitHub: {}\n",
            catalog_data
                .modules
                .get("loon_promax_github")
                .unwrap()
                .install_url
        ));
        readme.push(format!(
            "- **Shadowrocket PROMAX** CDN: {}\n",
            catalog_data
                .modules
                .get("shadowrocket_promax_cdn")
                .unwrap()
                .install_url
        ));
        readme.push(format!(
            "- **Shadowrocket PROMAX** GitHub: {}\n",
            catalog_data
                .modules
                .get("shadowrocket_promax_github")
                .unwrap()
                .install_url
        ));
        let readme_path = self.root_dir.join("rulesets/AdBlock/README.md");
        vec![
            crate::promax::artifact::GeneratedArtifact {
                target: catalog_path,
                content: format!("{}\n", json_str),
                kind: crate::promax::artifact::ArtifactKind::Metadata,
            },
            crate::promax::artifact::GeneratedArtifact {
                target: readme_path,
                content: readme.join(""),
                kind: crate::promax::artifact::ArtifactKind::Metadata,
            },
        ]
    }

    fn generate_module(
        &self,
        generated_rulesets: &[String],
        url_source: &str,
    ) -> crate::promax::artifact::GeneratedArtifact {
        let current_date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let complex_prefixes = ["URL-REGEX,", "USER-AGENT,", "PROCESS-NAME,", "DEST-PORT,"];

        let base_url = if url_source == "cdn" {
            CDN_BASE_URL
        } else {
            GITHUB_RAW_URL
        };
        let out_dir = if url_source == "cdn" {
            self.root_dir.join("modules/surge/head_expanse")
        } else {
            self.root_dir.join("modules/surge/head_expanse/github")
        };
        let _ = fs::create_dir_all(&out_dir);

        let base_filename =
            "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style)";
        let target_path = out_dir.join(format!("{}.sgmodule", base_filename));
        let shard_count = generated_rulesets.len();

        let mut rule_lines = Vec::new();
        rule_lines.push("# --- SYNCED PORT RULES START ---".to_string());
        rule_lines.push("# Automated update from ports source".to_string());

        let ports_source = self.root_dir.join("rulesets/Sources/DirectPorts.list");
        if ports_source.exists() {
            if let Ok(file) = File::open(&ports_source) {
                let reader = BufReader::new(file);
                for line in reader.lines().map_while(Result::ok) {
                    let stripped = line.trim();
                    if !stripped.is_empty() && !stripped.starts_with('#') {
                        rule_lines.push(stripped.to_string());
                    }
                }
            }
        }

        rule_lines.extend(vec![
            "# --- SYNCED PORT RULES END ---".to_string(),
            "".to_string(),
            "# High-Priority White-list (Prevent DNS failure)".to_string(),
            "DOMAIN,dns.alidns.com,DIRECT".to_string(),
            "DOMAIN,doh.pub,DIRECT".to_string(),
            "DOMAIN,dns.pub,DIRECT".to_string(),
            "DOMAIN,dot.pub,DIRECT".to_string(),
            "DOMAIN,doh.360.cn,DIRECT".to_string(),
            "DOMAIN,doh.dns.apple.com,DIRECT".to_string(),
            "# Block app-layer HTTPDNS first".to_string(),
            format!(
                "RULE-SET,{}rulesets/AdBlock/HTTPDNS_Hijack.list,REJECT",
                base_url
            ),
            "# Split REJECT Rulesets (purpose-grouped; see rulesets/AdBlock/README.md)".to_string(),
        ]);

        let mut shards_by_category: HashMap<String, Vec<String>> = HashMap::new();
        for rs_path in generated_rulesets {
            let cat = self.category_from_filename(rs_path.split('/').last().unwrap());
            shards_by_category
                .entry(cat.to_string())
                .or_default()
                .push(rs_path.clone());
        }

        for category in CATEGORIES {
            if let Some(paths) = shards_by_category.get_mut(*category) {
                if paths.is_empty() {
                    continue;
                }
                let meta = CATEGORY_META.get(*category).unwrap();
                rule_lines.push(format!("# ── {} · {} ──", meta.label_zh, meta.desc));
                paths.sort_by_key(|p| {
                    let name = p.split('/').last().unwrap();
                    let match_idx = Regex::new(r"_(\d+)\.list$").unwrap();
                    let shard_index = match_idx
                        .captures(name)
                        .and_then(|caps| caps[1].parse::<usize>().ok())
                        .unwrap_or(0);
                    (shard_index, name.to_string())
                });
                for rs_path in paths {
                    rule_lines.push(format!("RULE-SET,{}{},REJECT,extended-matching,pre-matching,update-interval=86400,no-resolve", base_url, rs_path));
                }
            }
        }

        rule_lines.extend(vec![
            "".to_string(),
            "# SKK Upstream Rulesets".to_string(),
            format!("RULE-SET,{}rulesets/Sources/skk_upstream/reject-no-drop.list,REJECT-NO-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve", base_url),
            format!("RULE-SET,{}rulesets/Sources/skk_upstream/reject-drop.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve", base_url),
            format!("RULE-SET,{}rulesets/Sources/skk_upstream/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve", base_url),
        ]);

        let mut complex_seen = HashSet::new();
        let active_cats: HashSet<&str> = CATEGORIES.iter().cloned().collect();
        for policy in &["REJECT", "REJECT-DROP", "REJECT-NO-DROP"] {
            let mut pool = HashSet::new();
            for cat in &active_cats {
                let empty_set = HashSet::new();
                let rules_in_cat = self
                    .rules
                    .get(*policy)
                    .unwrap()
                    .get(*cat)
                    .unwrap_or(&empty_set);
                pool.extend(rules_in_cat.clone());
            }
            if pool.is_empty() {
                continue;
            }
            let mut candidates = Vec::new();
            for r in pool {
                if *policy == "REJECT" {
                    if complex_prefixes.iter().any(|p| r.starts_with(p)) {
                        candidates.push(r);
                    }
                } else {
                    candidates.push(r);
                }
            }
            if candidates.is_empty() {
                continue;
            }
            rule_lines.push(format!(
                "# Merged {} complex rules ({})",
                policy,
                candidates.len()
            ));
            candidates.sort();
            for raw in candidates {
                if let Some(line) = self.attach_policy(&raw, policy) {
                    if complex_seen.insert(line.clone()) {
                        rule_lines.push(line);
                    }
                }
            }
        }

        let mut functional_sections = Vec::new();
        let mut func_counts = Vec::new();
        for sec_name in SECTION_ORDER {
            if *sec_name == "Rule"
                || *sec_name == "MITM"
                || *sec_name == "General"
                || *sec_name == "Host"
            {
                continue;
            }
            let lines = self.sections.get(*sec_name).unwrap();
            if lines.is_empty() {
                continue;
            }
            let deduped = crate::smart_cleanup::dedupe_section_lines_pub(sec_name, lines);
            if !deduped.is_empty() {
                functional_sections.push(crate::smart_cleanup::ModuleSectionPub {
                    Name: sec_name.to_string(),
                    Lines: deduped.clone(),
                });
                func_counts.push(format!("{}={}", sec_name, deduped.len()));
            }
        }

        if !self.mitm_hosts.is_empty() {
            let mut hosts: Vec<String> = self.mitm_hosts.iter().cloned().collect();
            hosts.sort();
            let mitm_line = format!("hostname = %APPEND% {}", hosts.join(", "));
            functional_sections.push(crate::smart_cleanup::ModuleSectionPub {
                Name: "MITM".to_string(),
                Lines: vec![mitm_line],
            });
            func_counts.push(format!("MITM={}", self.mitm_hosts.len()));
        }

        let mut all_sections = vec![crate::smart_cleanup::ModuleSectionPub {
            Name: "Rule".to_string(),
            Lines: rule_lines,
        }];
        all_sections.extend(functional_sections);
        all_sections = crate::smart_cleanup::merge_mitm_hosts_pub(&all_sections);
        let func_summary = if func_counts.is_empty() {
            "无脚本层".to_string()
        } else {
            func_counts.join(", ")
        };
        let name = format!(
            "🚫 Universal Ad-Blocking Rules (PROMAX) - [{}]",
            current_date
        );
        let desc = format!(
            "按用途分片({}片) + 应用内去广告({}); 索引 rulesets/AdBlock/catalog.json",
            shard_count, func_summary
        );
        let tag = "AdBlock, Dependency, HTTPDNS, Script".to_string();

        let mut header_meta = HashMap::new();
        header_meta.insert("name".to_string(), name);
        header_meta.insert("desc".to_string(), desc);
        header_meta.insert("author".to_string(), "ScriptHub-Automated".to_string());
        header_meta.insert(
            "icon".to_string(),
            "https://cdn.jsdelivr.net/gh/luestr/IconResource@main/Other_icon/120px/KeLee.png"
                .to_string(),
        );
        header_meta.insert("category".to_string(), GROUP_HEAD_EXPANSE.to_string());
        header_meta.insert("tag".to_string(), tag);
        header_meta.insert(
            "date".to_string(),
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        );

        let header_lines = crate::smart_cleanup::format_header_pub(&header_meta);
        let mut module_content =
            crate::smart_cleanup::format_module_pub(&header_lines, &all_sections, false);

        if url_source == "github" {
            let cdn_own = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/";
            let gh_own = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/";
            let mut lines_out = Vec::new();
            let mut in_url_rewrite = false;
            for line in module_content.lines() {
                let mut line = line.to_string();
                let stripped = line.trim();
                if stripped.starts_with('[') {
                    in_url_rewrite = stripped.to_lowercase().starts_with("[url rewrite]");
                }
                if !in_url_rewrite && line.contains(cdn_own) {
                    line = line.replace(cdn_own, gh_own);
                }
                lines_out.push(line);
            }
            module_content = lines_out.join("\n") + "\n";
        }

        crate::promax::artifact::GeneratedArtifact {
            target: target_path,
            content: module_content,
            kind: crate::promax::artifact::ArtifactKind::Surge,
        }
    }

    fn generate_loon_plugin(
        &self,
        generated_rulesets: &[String],
        url_source: &str,
    ) -> crate::promax::artifact::GeneratedArtifact {
        let current_date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let complex_prefixes = ["URL-REGEX,", "USER-AGENT,", "PROCESS-NAME,", "DEST-PORT,"];

        let base_url = if url_source == "cdn" {
            CDN_BASE_URL
        } else {
            GITHUB_RAW_URL
        };
        let out_dir = if url_source == "cdn" {
            self.root_dir.join("modules/loon")
        } else {
            self.root_dir.join("modules/loon/github")
        };
        let _ = fs::create_dir_all(&out_dir);

        let base_filename =
            "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style)";
        let target_path = out_dir.join(format!("{}.plugin", base_filename));
        let shard_count = generated_rulesets.len();

        let mut rule_lines = Vec::new();
        rule_lines.push("# --- SYNCED PORT RULES START ---".to_string());
        rule_lines.push("# Automated update from ports source".to_string());

        let ports_source = self.root_dir.join("rulesets/Sources/DirectPorts.list");
        if ports_source.exists() {
            if let Ok(file) = File::open(&ports_source) {
                let reader = BufReader::new(file);
                for line in reader.lines().map_while(Result::ok) {
                    let stripped = line.trim();
                    if !stripped.is_empty() && !stripped.starts_with('#') {
                        rule_lines.push(stripped.to_string());
                    }
                }
            }
        }

        rule_lines.extend(vec![
            "# --- SYNCED PORT RULES END ---".to_string(),
            "".to_string(),
            "# High-Priority White-list (Prevent DNS failure)".to_string(),
            "DOMAIN,dns.alidns.com,DIRECT".to_string(),
            "DOMAIN,doh.pub,DIRECT".to_string(),
            "DOMAIN,dns.pub,DIRECT".to_string(),
            "DOMAIN,dot.pub,DIRECT".to_string(),
            "DOMAIN,doh.360.cn,DIRECT".to_string(),
            "DOMAIN,doh.dns.apple.com,DIRECT".to_string(),
            "# Block app-layer HTTPDNS first".to_string(),
            format!(
                "RULE-SET,{}rulesets/AdBlock/HTTPDNS_Hijack.list,REJECT",
                base_url
            ),
            "# Split REJECT Rulesets (purpose-grouped; see rulesets/AdBlock/README.md)".to_string(),
        ]);

        let mut shards_by_category: HashMap<String, Vec<String>> = HashMap::new();
        for rs_path in generated_rulesets {
            let cat = self.category_from_filename(rs_path.split('/').last().unwrap());
            shards_by_category
                .entry(cat.to_string())
                .or_default()
                .push(rs_path.clone());
        }

        for category in CATEGORIES {
            if let Some(paths) = shards_by_category.get_mut(*category) {
                if paths.is_empty() {
                    continue;
                }
                let meta = CATEGORY_META.get(*category).unwrap();
                rule_lines.push(format!("# ── {} · {} ──", meta.label_zh, meta.desc));
                paths.sort();
                for rs_path in paths {
                    rule_lines.push(format!(
                        "RULE-SET,{}{},REJECT,no-resolve",
                        base_url, rs_path
                    ));
                }
            }
        }

        rule_lines.extend(vec![
            "".to_string(),
            "# SKK Upstream Rulesets".to_string(),
            format!("RULE-SET,{}rulesets/Sources/skk_upstream/reject-no-drop.list,REJECT-NO-DROP,no-resolve", base_url),
            format!("RULE-SET,{}rulesets/Sources/skk_upstream/reject-drop.list,REJECT-DROP,no-resolve", base_url),
            format!("RULE-SET,{}rulesets/Sources/skk_upstream/BlockHttpDNS.list,REJECT-DROP,no-resolve", base_url),
        ]);

        for policy in &["REJECT", "REJECT-DROP", "REJECT-NO-DROP"] {
            let mut pool = HashSet::new();
            for cat in CATEGORIES {
                let empty_set = HashSet::new();
                let rules_in_cat = self
                    .rules
                    .get(*policy)
                    .unwrap()
                    .get(*cat)
                    .unwrap_or(&empty_set);
                pool.extend(rules_in_cat.clone());
            }
            if pool.is_empty() {
                continue;
            }
            let mut candidates = Vec::new();
            for r in pool {
                if *policy == "REJECT" {
                    if complex_prefixes.iter().any(|p| r.starts_with(p)) {
                        candidates.push(r);
                    }
                } else {
                    candidates.push(r);
                }
            }
            if candidates.is_empty() {
                continue;
            }
            rule_lines.push(format!(
                "# Merged {} complex rules ({})",
                policy,
                candidates.len()
            ));
            candidates.sort();
            for raw in candidates {
                if let Some(line) = self.attach_policy(&raw, policy) {
                    rule_lines.push(line);
                }
            }
        }

        let mut rewrite_lines = Vec::new();
        let mut func_counts = Vec::new();
        for sec_name in &["URL Rewrite", "Map Local", "Body Rewrite", "Header Rewrite"] {
            let lines = self.sections.get(*sec_name).unwrap();
            if lines.is_empty() {
                continue;
            }
            let deduped = crate::smart_cleanup::dedupe_section_lines_pub(sec_name, lines);
            if !deduped.is_empty() {
                for line in &deduped {
                    let converted = if *sec_name == "Body Rewrite" {
                        crate::smart_cleanup::adapt_body_rewrite_line_for_loon_pub(line)
                    } else {
                        crate::smart_cleanup::adapt_rewrite_line_for_loon_pub(line)
                    };
                    if !converted.is_empty() {
                        rewrite_lines.push(converted);
                    }
                }
                func_counts.push(format!("{}={}", sec_name, deduped.len()));
            }
        }

        let mut script_lines = Vec::new();
        let s_lines = self.sections.get("Script").unwrap();
        if !s_lines.is_empty() {
            let deduped = crate::smart_cleanup::dedupe_section_lines_pub("Script", s_lines);
            if !deduped.is_empty() {
                for line in &deduped {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        script_lines.push(line.clone());
                        continue;
                    }
                    if let Some(idx) = trimmed.find('=') {
                        let tag = trimmed[..idx].trim().to_string();
                        let content = trimmed[idx + 1..].trim().to_string();
                        let converted =
                            crate::smart_cleanup::adapt_script_line_for_loon_pub(&tag, &content);
                        if !converted.is_empty() {
                            script_lines.push(converted);
                        } else {
                            script_lines.push(line.clone());
                        }
                    } else {
                        script_lines.push(line.clone());
                    }
                }
                func_counts.push(format!("Script={}", deduped.len()));
            }
        }

        let mut mitm_lines = Vec::new();
        if !self.mitm_hosts.is_empty() {
            let mut hosts: Vec<String> = self.mitm_hosts.iter().cloned().collect();
            hosts.sort();
            mitm_lines.push(format!("hostname = {}", hosts.join(", ")));
            func_counts.push(format!("MITM={}", self.mitm_hosts.len()));
        }

        let func_summary = if func_counts.is_empty() {
            "无脚本层".to_string()
        } else {
            func_counts.join(", ")
        };
        let name = format!(
            "🚫 Universal Ad-Blocking Rules (PROMAX) - [{}]",
            current_date
        );
        let desc = format!(
            "按用途分片({}片) + 应用内去广告({}); 索引 rulesets/AdBlock/catalog.json",
            shard_count, func_summary
        );

        let mut output = Vec::new();
        output.push(format!("#!name={}", name));
        output.push(format!("#!desc={}", desc));
        output.push("#!author=ScriptHub-Automated".to_string());
        output.push("#!icon=https://cdn.jsdelivr.net/gh/luestr/IconResource@main/Other_icon/120px/KeLee.png".to_string());
        output.push(format!(
            "#!date={}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ));
        output.push(String::new());

        if !rule_lines.is_empty() {
            output.push("[Rule]".to_string());
            output.extend(rule_lines);
            output.push(String::new());
        }

        if !rewrite_lines.is_empty() {
            output.push("[Rewrite]".to_string());
            output.extend(rewrite_lines);
            output.push(String::new());
        }

        if !script_lines.is_empty() {
            output.push("[Script]".to_string());
            output.extend(script_lines);
            output.push(String::new());
        }

        if !mitm_lines.is_empty() {
            output.push("[MitM]".to_string());
            output.extend(mitm_lines);
            output.push(String::new());
        }

        let mut plugin_content = output.join("\n");
        if url_source == "github" {
            let cdn_own = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/";
            let gh_own = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/";
            let mut lines_out = Vec::new();
            let mut in_url_rewrite = false;
            for line in plugin_content.lines() {
                let mut line = line.to_string();
                let stripped = line.trim();
                if stripped.starts_with('[') {
                    in_url_rewrite = stripped.to_lowercase().starts_with("[rewrite]");
                }
                if !in_url_rewrite && line.contains(cdn_own) {
                    line = line.replace(cdn_own, gh_own);
                }
                lines_out.push(line);
            }
            plugin_content = lines_out.join("\n") + "\n";
        }

        crate::promax::artifact::GeneratedArtifact {
            target: target_path,
            content: plugin_content,
            kind: crate::promax::artifact::ArtifactKind::Loon,
        }
    }

    fn attach_policy(&self, rule: &str, policy: &str) -> Option<String> {
        let mut parsed = crate::promax::rule::SurgeRule::parse(rule).ok()?;
        parsed.policy = Some(policy.to_ascii_uppercase());
        Some(parsed.render_module(policy))
    }

    pub fn merge(&mut self, execute: bool) -> bool {
        println!(
            "\x1b[0;32m[INFO]\x1b[0m ============================================================"
        );
        println!("\x1b[0;32m[INFO]\x1b[0m 🚀 AdBlock Module Consolidation (Canonical Rebuild)");
        println!(
            "\x1b[0;32m[INFO]\x1b[0m ============================================================"
        );

        if let Err(error) = self.load_whitelist() {
            println!(
                "\x1b[0;31m[ERROR]\x1b[0m Refusing PROMAX build without safety whitelist: {error}"
            );
            return false;
        }
        self.load_hashes();

        let mut source_entries = self.load_source_entries();

        let priority_map: HashMap<String, usize> = CATEGORIES
            .iter()
            .enumerate()
            .map(|(i, &c)| (c.to_string(), i))
            .collect();
        source_entries.sort_by_key(|e| {
            let cat = self.determine_category(&e.source);
            *priority_map.get(cat).unwrap_or(&99)
        });

        let remote_contents = match self.download_remote_contents(&source_entries) {
            Ok(contents) => contents,
            Err(failures) => {
                for failure in &failures {
                    println!(
                        "\x1b[0;31m[ERROR]\x1b[0m PROMAX source download failed: {} ({})",
                        failure.url, failure.error
                    );
                }
                println!(
                    "\x1b[0;31m[ERROR]\x1b[0m Refusing partial PROMAX build after {} download failure(s).",
                    failures.len()
                );
                return false;
            }
        };

        println!("\x1b[0;32m[INFO]\x1b[0m Integrating Local AdBlock Sources");
        let local_dir = self.root_dir.join("modules/source/local");
        if local_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&local_dir) {
                let mut files: Vec<PathBuf> =
                    entries.map_while(Result::ok).map(|e| e.path()).collect();
                files.sort();
                for f in files {
                    let name = f.file_name().unwrap().to_string_lossy().to_string();
                    if name.ends_with(".sgmodule") || name.ends_with(".module") {
                        println!(
                            "\x1b[0;32m[INFO]\x1b[0m   + Rules from local source: {}",
                            name
                        );
                        self.extract_from_file(&f, "REJECT", "Local", false, true);
                    }
                }
            }
        }

        println!("\x1b[0;32m[INFO]\x1b[0m Integrating App AdBlock Rules (LocalModules)");
        let local_modules_dir = self.root_dir.join("rulesets/Sources/LocalModules");
        if local_modules_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&local_modules_dir) {
                let mut files: Vec<PathBuf> =
                    entries.map_while(Result::ok).map(|e| e.path()).collect();
                files.sort();
                for f in files {
                    let name = f.file_name().unwrap().to_string_lossy().to_string();
                    if !name.ends_with(".sgmodule") && !name.ends_with(".module") {
                        continue;
                    }
                    let mode = self.resolve_module_ingest_mode(&f, false);
                    if mode == "skip" {
                        continue;
                    }
                    println!(
                        "\x1b[0;32m[INFO]\x1b[0m   + Rules from LocalModules ({}): {}",
                        mode, name
                    );
                    self.extract_from_file(&f, "REJECT", "Local", false, true);
                }
            }
        }

        println!("\x1b[0;32m[INFO]\x1b[0m Fetching Canonical AdBlock Sources");
        let failures = self.process_source_entries(&source_entries, &remote_contents);

        if !failures.is_empty() {
            println!(
                "\x1b[0;31m[ERROR]\x1b[0m Refusing partial PROMAX build after {} source failure(s).",
                failures.len()
            );
            return false;
        }

        if execute {
            self.functional_stats = self.integrate_functional_sections(&remote_contents);
            let (generated_rulesets, mut artifacts) = self.generate_ruleset();
            if let Err(error) =
                crate::promax::artifact::validate_coverage_floor(&self.root_dir, &artifacts)
            {
                println!("\x1b[0;31m[ERROR]\x1b[0m Refusing PROMAX coverage regression: {error}");
                return false;
            }
            artifacts.extend(self.generate_catalog_artifacts(&generated_rulesets, &artifacts));
            artifacts.append(&mut self.split_artifacts);

            println!(
                "\x1b[0;32m[INFO]\x1b[0m Rendering PROMAX Surge, Loon, and Shadowrocket artifacts..."
            );
            let surge_cdn = self.generate_module(&generated_rulesets, "cdn");
            let surge_github = self.generate_module(&generated_rulesets, "github");
            let shadowrocket_cdn =
                match crate::promax::artifact::shadowrocket_variant(&self.root_dir, &surge_cdn) {
                    Ok(artifact) => artifact,
                    Err(error) => {
                        println!(
                            "\x1b[0;31m[ERROR]\x1b[0m Shadowrocket conversion failed: {error}"
                        );
                        return false;
                    }
                };
            let shadowrocket_github = match crate::promax::artifact::shadowrocket_variant(
                &self.root_dir,
                &surge_github,
            ) {
                Ok(artifact) => artifact,
                Err(error) => {
                    println!("\x1b[0;31m[ERROR]\x1b[0m Shadowrocket conversion failed: {error}");
                    return false;
                }
            };
            artifacts.extend([
                surge_cdn,
                surge_github,
                self.generate_loon_plugin(&generated_rulesets, "cdn"),
                self.generate_loon_plugin(&generated_rulesets, "github"),
                shadowrocket_cdn,
                shadowrocket_github,
            ]);
            if let Err(error) = crate::promax::artifact::publish_artifacts(
                &self.root_dir,
                &artifacts,
                &self.quarantine,
            ) {
                println!("\x1b[0;31m[ERROR]\x1b[0m Refusing PROMAX publication: {error}");
                return false;
            }
            self.prune_stale_rulesets(&generated_rulesets);
            println!(
                "\x1b[0;32m[SUCCESS]\x1b[0m Published validated PROMAX artifacts as one transaction."
            );

            let mut local_source_paths = Vec::new();
            for e in &source_entries {
                if !e.is_remote() {
                    local_source_paths.push(e.resolved_path(&self.root_dir));
                }
            }

            // Build input hashes
            let mut unique_local = HashSet::new();
            for p in local_source_paths {
                unique_local.insert(p);
            }

            let mut input_hashes = HashMap::new();
            for p in unique_local {
                if p.exists() {
                    if let Ok(rel) = p.strip_prefix(&self.root_dir) {
                        if let Ok(bytes) = fs::read(&p) {
                            let digest = md5::compute(bytes);
                            input_hashes
                                .insert(rel.to_string_lossy().to_string(), format!("{:x}", digest));
                        }
                    }
                }
            }
            self.save_hashes(&input_hashes);
        } else {
            println!("\x1b[0;32m[INFO]\x1b[0m Dry Run Mode - Generation skipped.");
        }

        println!(
            "\x1b[0;32m[SUCCESS]\x1b[0m AdBlock merge completed successfully without failures."
        );

        true
    }
}

pub fn run_adblock_manager(root_dir: &str, execute: bool) -> bool {
    let root_path = PathBuf::from(root_dir);
    let mut manager = AdBlockManager::new(root_path);
    manager.merge(execute)
}

#[cfg(test)]
mod tests {
    use super::AdBlockManager;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn offline_fixture_publishes_complete_promax_product_set() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-offline-{unique}"));
        let links = root.join("rulesets/Sources/Links");
        fs::create_dir_all(&links).unwrap();

        let mut whitelist = String::new();
        for index in 0..300 {
            whitelist.push_str(&format!("DOMAIN-SUFFIX,protected{index}.example\n"));
        }
        for index in 0..200 {
            whitelist.push_str(&format!(
                "IP-CIDR,10.{}.{}.0/24\n",
                index / 256,
                index % 256
            ));
        }
        fs::write(
            root.join("rulesets/Sources/adblock_whitelist.list"),
            whitelist,
        )
        .unwrap();

        fs::write(
            links.join("AdBlock_sources.list"),
            "../fixture.list | REJECT | auto\n",
        )
        .unwrap();
        fs::write(
            root.join("rulesets/Sources/fixture.list"),
            "DOMAIN-SUFFIX,ads.fixture.test\nDOMAIN,tracker.fixture.test\nIP-CIDR,192.0.2.0/24\n",
        )
        .unwrap();
        fs::write(
            links.join("AdBlock_functional_sources.list"),
            "../fixture.sgmodule\n",
        )
        .unwrap();
        fs::write(
            root.join("rulesets/Sources/fixture.sgmodule"),
            r#"#!name=Fixture AdBlock
[Rule]
DOMAIN-SUFFIX,functional-ad.fixture.test,REJECT
[URL Rewrite]
^https://ads\.fixture\.test/splash _ reject
[Script]
fixture = type=http-response,pattern=^https://ads\.fixture\.test/feed,requires-body=1,script-path=https://scripts.fixture.test/remove.js
[Body Rewrite]
http-response-jq ^https:\/\/ads\.fixture\.test\/feed 'del(.data.ad)'
[MITM]
hostname = ads.fixture.test
"#,
        )
        .unwrap();

        assert!(AdBlockManager::new(root.clone()).merge(true));

        let products = [
            "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
            "modules/surge/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
            "modules/loon/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).plugin",
            "modules/loon/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).plugin",
            "modules/shadowrocket/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module",
            "modules/shadowrocket/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module",
        ];
        for product in products {
            let content = fs::read_to_string(root.join(product)).unwrap();
            assert!(content.contains("scripts.fixture.test/remove.js"));
        }
        assert!(root.join("rulesets/AdBlock/catalog.json").is_file());
        assert!(root.join("rulesets/AdBlock/README.md").is_file());
        assert!(root.join("rulesets/AdBlock/quarantine.json").is_file());
        let shard = fs::read_to_string(root.join("rulesets/AdBlock/AdBlock_Local.list")).unwrap();
        assert!(shard.contains("functional-ad.fixture.test"));

        fs::remove_dir_all(root).unwrap();
    }
}
