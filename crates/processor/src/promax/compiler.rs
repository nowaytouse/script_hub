use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

static SHARD_INDEX_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"_(\d+)\.list$").unwrap());

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
    "Privacy_CNAME",
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
        "Privacy_CNAME",
        CategoryMeta {
            label_zh: "CNAME 伪装追踪",
            desc: "AdGuard CNAME 与邮件追踪域名",
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
        if i > 0 && (len - i).is_multiple_of(3) {
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
    modules: BTreeMap<String, CatalogItem>,
    manifest: String,
    functional_manifest: String,
    functional_sections: BTreeMap<String, usize>,
    #[serde(default)]
    verified_rolling_feeds: BTreeMap<String, super::feed::VerifiedFeed>,
    shards: Vec<ShardItem>,
}

fn render_catalog_readme(catalog: &CatalogJson) -> String {
    let mut readme = format!(
        "# AdBlock\n\n[English](README.md) · [简体中文](README.zh-CN.md)\n\n> Generated `{}` · [`catalog.json`](catalog.json)\n\n| Category | File | Rules | CDN | GitHub |\n| --- | --- | --- | --- | --- |\n",
        catalog.updated
    );
    for item in &catalog.shards {
        readme.push_str(&format!(
            "| {} | `{}` | {} | [CDN]({}) | [GitHub]({}) |\n",
            item.category,
            item.file,
            format_commas(item.rule_count),
            item.cdn_url,
            item.github_url
        ));
    }
    readme.push_str("\n## Modules\n\n");
    for (key, item) in &catalog.modules {
        readme.push_str(&format!("- [{key}]({})\n", item.install_url));
    }
    readme
}

#[derive(Clone)]
struct SourceEntry {
    manifest_path: PathBuf,
    source: String,
    policy: String,
    kind: String,
    category: Option<String>,
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
                .next_back()
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
    source_exceptions: HashSet<String>,                       // AdGuard/ABP @@ domain exceptions
    verified_rolling_feeds: BTreeMap<String, super::feed::VerifiedFeed>,
    sections: HashMap<String, Vec<String>>, // section_name -> lines
    section_seen: HashMap<String, HashSet<String>>, // section_name -> seen keys
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
        let root_dir = root_dir.canonicalize().unwrap_or(root_dir);
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
            source_exceptions: HashSet::new(),
            verified_rolling_feeds: BTreeMap::new(),
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
        let apple_path = self.root_dir.join("rulesets/RULE-SET/Apple.list");
        let apple_content = fs::read_to_string(&apple_path).map_err(|error| {
            format!(
                "required Apple safety ruleset {} is unreadable: {error}",
                apple_path.display()
            )
        })?;
        let apple_network_rules = apple_content.lines().filter(|line| {
            crate::promax::rule::SurgeRule::parse(line).is_ok_and(|rule| {
                !crate::is_ruleset_watermark(&rule.payload)
                    && matches!(
                        rule.kind,
                        crate::promax::rule::RuleKind::Domain
                            | crate::promax::rule::RuleKind::DomainSuffix
                            | crate::promax::rule::RuleKind::DomainKeyword
                            | crate::promax::rule::RuleKind::IpCidr
                            | crate::promax::rule::RuleKind::IpCidr6
                    )
            })
        });
        let (safety, stats) = crate::promax::safety::SafetyPolicy::from_lines_with_stats(
            content.lines().chain(apple_network_rules),
        );
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
                let category = parts.get(3).filter(|value| !value.is_empty()).cloned();

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
                if category
                    .as_deref()
                    .is_some_and(|value| !CATEGORIES.contains(&value))
                {
                    println!(
                        "\x1b[0;33m[WARN]\x1b[0m Skipping invalid category in AdBlock manifest: {}",
                        line
                    );
                    continue;
                }

                let entry = SourceEntry {
                    manifest_path: path.clone(),
                    source,
                    policy,
                    kind,
                    category,
                };
                let key = format!(
                    "{}|{}|{}|{}",
                    entry.source,
                    entry.policy,
                    entry.kind,
                    entry.category.as_deref().unwrap_or_default()
                );
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
        let (functional_paths, functional_urls) = self.load_functional_source_paths();
        urls.extend(functional_urls);

        println!(
            "\x1b[0;32m[INFO]\x1b[0m Rust downloader resolving {} remote PROMAX source(s)",
            urls.len()
        );
        let downloader = crate::promax::source::Downloader::new(
            crate::promax::source::DownloadConfig::default(),
        );
        let batch = downloader.download_many(urls);
        if !batch.failures.is_empty() {
            return Err(batch.failures);
        }
        // Probe maintained replacements and explicitly licensed app dependencies,
        // not every embedded script or any client-gated resource.
        let mut resources = std::collections::BTreeSet::new();
        for body in batch.contents.values() {
            if body.contains("[Script]") {
                resources.extend(super::script_resources::required_resources(body));
            }
        }
        for (path, _) in functional_paths {
            let body = fs::read_to_string(&path).map_err(|error| {
                vec![crate::promax::source::DownloadFailure {
                    url: path
                        .strip_prefix(&self.root_dir)
                        .unwrap_or(&path)
                        .display()
                        .to_string(),
                    error: format!("cannot inspect functional script dependencies: {error}"),
                }]
            })?;
            resources.extend(super::script_resources::required_resources(&body));
        }
        if !resources.is_empty() {
            println!(
                "[INFO] validating {} maintained script dependency resource(s)",
                resources.len()
            );
            let probe =
                crate::promax::source::Downloader::new(crate::promax::source::DownloadConfig {
                    attempts: 2,
                    timeout: std::time::Duration::from_secs(30),
                    batch_timeout: std::time::Duration::from_secs(65),
                    backoff: std::time::Duration::from_millis(250),
                    concurrency: 2,
                    max_bytes: 2 * 1024 * 1024,
                    max_total_bytes: 4 * 1024 * 1024,
                })
                .download_many(resources);
            let mut failures = probe.failures;
            for (url, body) in probe.contents {
                if let Err(error) = super::script_resources::validate_script_body(&body) {
                    failures.push(crate::promax::source::DownloadFailure { url, error });
                }
            }
            if !failures.is_empty() {
                return Err(failures);
            }
        }
        Ok(batch.contents)
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

    fn add_rule_line(&mut self, line: &str, default_policy: &str, category: &str) {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            return;
        }
        if let Some(exception) = stripped.strip_prefix("@@") {
            if let Some(rule) = crate::promax::functional::normalize_domain_source_line(exception)
                && let Ok(parsed) = crate::promax::rule::SurgeRule::parse(&rule)
            {
                self.source_exceptions.insert(parsed.payload);
            }
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
        let cleaned = crate::strip_inline_comment(stripped);
        let parse_candidate = if cleaned.contains(',') {
            Cow::Borrowed(cleaned.as_str())
        } else {
            let Some(normalized) =
                crate::promax::functional::normalize_domain_source_line(&cleaned)
            else {
                return;
            };
            Cow::Owned(normalized)
        };
        let mut parsed = match crate::promax::rule::SurgeRule::parse(&parse_candidate) {
            Ok(rule) => rule,
            Err(parse_error) => {
                self.quarantine
                    .push(crate::promax::safety::QuarantineRecord {
                        source: self.current_source.clone(),
                        line: self.current_line,
                        candidate: stripped.to_string(),
                        reason: parse_error.to_string(),
                        risk: crate::promax::safety::RiskClass::Invalid,
                    });
                return;
            }
        };
        if crate::is_ruleset_watermark(&parsed.payload) {
            self.quarantine
                .push(crate::promax::safety::QuarantineRecord {
                    source: self.current_source.clone(),
                    line: self.current_line,
                    candidate: stripped.to_string(),
                    reason: "upstream-ruleset-watermark".to_string(),
                    risk: crate::promax::safety::RiskClass::Invalid,
                });
            return;
        }
        let policy = parsed
            .policy
            .as_deref()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(default_policy.to_ascii_uppercase()));
        let policy = match policy.as_ref() {
            "PROXY" => {
                self.quarantine
                    .push(crate::promax::safety::QuarantineRecord {
                        source: self.current_source.clone(),
                        line: self.current_line,
                        candidate: stripped.to_string(),
                        reason: "unsupported-proxy-policy".to_string(),
                        risk: crate::promax::safety::RiskClass::Invalid,
                    });
                return;
            }
            "REJECT-TINYGIF" | "REJECT-IMG" => "REJECT",
            policy => policy,
        };

        if let crate::promax::safety::SafetyDecision::Quarantine(reason) =
            self.safety.decision(&parsed)
        {
            self.quarantine
                .push(crate::promax::safety::QuarantineRecord {
                    source: self.current_source.clone(),
                    line: self.current_line,
                    candidate: stripped.to_string(),
                    reason: reason.to_string(),
                    risk: crate::promax::safety::RiskClass::ProtectedService,
                });
            return;
        }

        if !category.starts_with("ThreatIntel")
            && category != "Security"
            && !crate::promax::safety::is_reviewed_exact_privacy_domain(&parsed)
            && matches!(
                parsed.kind,
                crate::promax::rule::RuleKind::Domain | crate::promax::rule::RuleKind::DomainSuffix
            )
            && let crate::promax::safety::SafetyDecision::Quarantine(reason) =
                crate::promax::safety::classify_domain_payload(&parsed.payload)
        {
            self.quarantine
                .push(crate::promax::safety::QuarantineRecord {
                    source: self.current_source.clone(),
                    line: self.current_line,
                    candidate: stripped.to_string(),
                    reason: reason.to_string(),
                    risk: crate::promax::safety::RiskClass::BroadMedia,
                });
            return;
        }

        let rendered = parsed.render_external();

        // Deduplication
        let seen_bucket = self.seen_rules.get_mut(policy).unwrap();
        if seen_bucket.contains(&rendered) {
            return;
        }

        // Validated domain payloads are ASCII; borrow ancestor slices instead of joining labels.
        if matches!(
            parsed.kind,
            crate::promax::rule::RuleKind::Domain | crate::promax::rule::RuleKind::DomainSuffix
        ) {
            parsed.payload.make_ascii_lowercase();
            let policy_suffixes = self.seen_suffixes.get_mut(policy).unwrap();
            if parsed.kind == crate::promax::rule::RuleKind::Domain {
                let mut candidate = parsed.payload.as_str();
                while let Some((_, parent)) = candidate.split_once('.') {
                    if policy_suffixes.contains(candidate) {
                        return;
                    }
                    candidate = parent;
                }
            } else {
                policy_suffixes.insert(parsed.payload);
            }
        }

        let cat_bucket = self
            .rules
            .get_mut(policy)
            .unwrap()
            .get_mut(category)
            .unwrap();
        cat_bucket.insert(rendered.clone());
        seen_bucket.insert(rendered);
    }

    fn add_section_line(&mut self, section_name: &str, line: &str) {
        if !self.sections.contains_key(section_name) {
            return;
        }
        let normalized;
        let line = if section_name == "Script" {
            normalized = super::script_resources::normalize_script_line(line);
            normalized.as_str()
        } else {
            line
        };
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            return;
        }

        let syntax_errors =
            crate::promax::validation::validate_surge_section_line(section_name, stripped);
        if let Some(first_error) = syntax_errors.first() {
            self.quarantine
                .push(crate::promax::safety::QuarantineRecord {
                    source: self.current_source.clone(),
                    line: self.current_line,
                    candidate: stripped.to_string(),
                    reason: format!("invalid-functional-syntax:{}", first_error.code),
                    risk: crate::promax::safety::RiskClass::Invalid,
                });
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
        let matcher_key = (section_name == "Script")
            .then(|| crate::smart_cleanup::script_matcher_key_pub(stripped))
            .flatten();
        if seen.contains(&key) || matcher_key.as_ref().is_some_and(|key| seen.contains(key)) {
            return;
        }
        seen.insert(key);
        seen.extend(matcher_key);
        self.sections
            .get_mut(section_name)
            .unwrap()
            .push(stripped.to_string());
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
        let host_patterns =
            crate::promax::safety::SafetyPolicy::functional_host_patterns(&functional_lines);
        let removed: Vec<String> = self
            .mitm_hosts
            .iter()
            .filter(|hostname| {
                !self
                    .safety
                    .hostname_is_referenced_by_patterns(hostname, &host_patterns)
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
        self.mitm_hosts = self
            .safety
            .refine_mitm_hosts(
                &self.mitm_hosts.iter().cloned().collect::<Vec<_>>(),
                &host_patterns,
            )
            .into_iter()
            .collect();
        self.mitm_hosts
            .extend(crate::promax::safety::apple_mitm_exclusions());
        self.mitm_hosts
            .retain(|host| host.starts_with('-') || !crate::mitm_manager::is_restricted(host));
        let normalized =
            crate::smart_cleanup::merge_mitm_hosts_pub(&[crate::smart_cleanup::ModuleSectionPub {
                name: "MITM".to_string(),
                lines: vec![format!(
                    "hostname = %APPEND% {}",
                    self.mitm_hosts
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )],
            }]);
        if let Some(line) = normalized.first().and_then(|section| section.lines.first()) {
            self.mitm_hosts = line
                .split_once('=')
                .map(|(_, hosts)| hosts.trim().trim_start_matches("%APPEND%").trim())
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .map(str::to_string)
                .collect();
        }
    }

    fn optimize_runtime_sections(&mut self) {
        for section in [
            "URL Rewrite",
            "Map Local",
            "Script",
            "Body Rewrite",
            "Header Rewrite",
        ] {
            let lines = self.sections.get_mut(section).unwrap();
            *lines = lines
                .iter()
                .map(|line| crate::promax::functional::narrow_functional_authority(section, line))
                .collect();
        }
        // Surge runs only the first matching script per HTTP event; retain that priority.
        let scripts = self.sections.get_mut("Script").unwrap();
        *scripts = crate::smart_cleanup::dedupe_section_lines_pub("Script", scripts);
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
        let mut current_section = "";
        let mut in_rule_section = false;
        let mut seen_real_header = false;

        self.current_source = source.to_string();
        for (index, line) in text.lines().enumerate() {
            self.current_line = index + 1;
            let stripped = line.trim();
            if stripped.is_empty() {
                continue;
            }

            if stripped.starts_with('[') && stripped.ends_with(']') && !stripped.starts_with('#') {
                current_section = &stripped[1..stripped.len() - 1];
                in_rule_section = matches!(current_section, "Rule" | "Adblock Plus");
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
                if stripped.starts_with("@@")
                    || crate::promax::rule::SurgeRule::parse(stripped).is_ok()
                    || crate::promax::functional::normalize_domain_source_line(stripped).is_some()
                {
                    self.add_rule_line(stripped, default_policy, category);
                    continue;
                }
                if include_sections && let Some(inferred) = self.infer_non_rule_section(stripped) {
                    if inferred == "MITM" && stripped.starts_with("hostname") {
                        let hosts = stripped
                            .split_once('=')
                            .map(|x| x.1)
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
                continue;
            }

            if in_rule_section {
                self.add_rule_line(stripped, default_policy, category);
            } else if include_sections && !stripped.starts_with('#') {
                if self.sections.contains_key(current_section) {
                    self.add_section_line(current_section, stripped);
                } else if current_section == "MITM" && stripped.starts_with("hostname") {
                    let hosts = stripped
                        .split_once('=')
                        .map(|x| x.1)
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
        let mut category_sources = HashMap::<String, usize>::new();
        self.verified_rolling_feeds.clear();
        for entry in source_entries {
            let category = entry
                .category
                .clone()
                .unwrap_or_else(|| self.determine_category(&entry.source).to_string());
            *category_sources.entry(category.clone()).or_default() += 1;
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

            if entry.policy == "REJECT" {
                match super::feed::verify_rolling_feed(&entry.source, &category, mod_text) {
                    Ok(Some(feed)) => {
                        println!(
                            "[INFO] Verified rolling source {} version {}: {} declared entries match its complete body.",
                            feed.source, feed.version, feed.declared_entries
                        );
                        self.verified_rolling_feeds.insert(category.clone(), feed);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        failures.push(format!("{}: {error}", entry.display_name(&self.root_dir)));
                        continue;
                    }
                }
            }

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
        self.verified_rolling_feeds
            .retain(|category, _| category_sources.get(category) == Some(&1));
        failures
    }

    fn filter_rules(&self, rules: &HashSet<String>) -> Result<Vec<String>, String> {
        let exceptions = regex::RegexSet::new(self.source_exceptions.iter().map(|domain| {
            let pattern = regex::escape(domain)
                .replace("\\*", ".*")
                .replace("\\?", ".");
            format!("(?i)(?:^|\\.){pattern}$")
        }))
        .map_err(|error| format!("cannot compile AdGuard domain exceptions: {error}"))?;
        let mut filtered = Vec::new();
        for rule in rules {
            if let Ok(parsed) = crate::promax::rule::SurgeRule::parse(rule)
                && matches!(
                    parsed.kind,
                    crate::promax::rule::RuleKind::Domain
                        | crate::promax::rule::RuleKind::DomainSuffix
                        | crate::promax::rule::RuleKind::DomainWildcard
                )
                && exceptions.is_match(&parsed.payload)
            {
                continue;
            }
            filtered.push(rule.clone());
        }
        filtered.sort();
        Ok(filtered)
    }

    fn generate_ruleset(
        &self,
    ) -> Result<(Vec<String>, Vec<crate::promax::artifact::GeneratedArtifact>), String> {
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
            let rules = self.filter_rules(rules_in_cat)?;

            if rules.is_empty() {
                println!(
                    "\x1b[0;32m[INFO]\x1b[0m   ⊘ Skip shard for {}: no unique rules",
                    category
                );
                continue;
            }

            let meta = CATEGORY_META.get(category).unwrap();
            let mut domains = Vec::new();
            let mut ips = Vec::new();
            let mut other = Vec::new();
            for rule in rules {
                match crate::promax::rule::SurgeRule::parse(&rule).map(|parsed| parsed.kind) {
                    Ok(
                        crate::promax::rule::RuleKind::Domain
                        | crate::promax::rule::RuleKind::DomainSuffix
                        | crate::promax::rule::RuleKind::DomainKeyword
                        | crate::promax::rule::RuleKind::DomainWildcard
                        | crate::promax::rule::RuleKind::DomainRegex,
                    ) => domains.push(rule),
                    Ok(
                        crate::promax::rule::RuleKind::IpCidr
                        | crate::promax::rule::RuleKind::IpCidr6
                        | crate::promax::rule::RuleKind::IpAsn
                        | crate::promax::rule::RuleKind::GeoIp,
                    ) => ips.push(rule),
                    _ => other.push(rule),
                }
            }

            for (suffix, part_rules) in [("", domains), ("_ip", ips), ("_other_extra", other)] {
                let chunk_count = part_rules.len().div_ceil(RULES_PER_SHARD);
                for (idx, chunk) in part_rules.chunks(RULES_PER_SHARD).enumerate() {
                    let filename = if suffix.is_empty() {
                        if chunk_count == 1 {
                            format!("AdBlock_{}.list", category)
                        } else {
                            format!("AdBlock_{}_{:02}.list", category, idx + 1)
                        }
                    } else if chunk_count == 1 {
                        format!("AdBlock_{}{}.list", category, suffix)
                    } else {
                        format!("AdBlock_{}{}_{:02}.list", category, suffix, idx + 1)
                    };
                    let shard_label = if chunk_count == 1 {
                        "1/1".to_string()
                    } else {
                        format!("{}/{}", idx + 1, chunk_count)
                    };

                    let mut content = Vec::new();
                    content.push(format!("# Ruleset: AdBlock · {}", meta.label_zh));
                    content.push(format!("# Purpose: {}", meta.desc));
                    content.push(format!("# Category-Key: {}", category));
                    content.push(format!("# Shard: {}", shard_label));
                    content.push(format!("# Total Rules: {}", chunk.len()));
                    content.extend(chunk.iter().cloned());

                    let mut out_str = content.join("\n");
                    out_str.push('\n');
                    let rel_path = format!("rulesets/AdBlock/{filename}");
                    artifacts.push(crate::promax::artifact::GeneratedArtifact {
                        target: adblock_dir.join(&filename),
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
        }

        Ok((generated_files, artifacts))
    }

    fn stale_ruleset_shard_paths(
        &self,
        generated_files: &[String],
    ) -> Result<Vec<PathBuf>, String> {
        let catalog_path = self.root_dir.join("rulesets/AdBlock/catalog.json");
        let catalog = match fs::read_to_string(&catalog_path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("cannot read previous shard catalog: {error}")),
        };
        let value: serde_json::Value = serde_json::from_str(&catalog)
            .map_err(|error| format!("cannot parse previous shard catalog: {error}"))?;
        let shards = value["shards"]
            .as_array()
            .ok_or_else(|| "previous shard catalog is missing a shards array".to_string())?;
        let mut retired = HashSet::new();
        for shard in shards {
            let file = shard["file"]
                .as_str()
                .ok_or_else(|| "previous shard catalog contains a missing file path".to_string())?;
            let name = file.strip_prefix("rulesets/AdBlock/").ok_or_else(|| {
                format!("previous shard path is outside rulesets/AdBlock: {file}")
            })?;
            if name.contains('/')
                || name.contains('\\')
                || !Self::is_known_shard_filename(name)
                || Path::new(file).parent() != Some(Path::new("rulesets/AdBlock"))
            {
                return Err(format!(
                    "previous shard path is not a direct AdBlock shard: {file}"
                ));
            }
            if !generated_files.iter().any(|generated| generated == file) {
                retired.insert(file.to_string());
            }
        }
        Ok(retired
            .into_iter()
            .map(|file| self.root_dir.join(file))
            .collect())
    }

    fn prune_legacy_and_stale_rulesets(&self, stale_shards: &[PathBuf]) -> Result<(), String> {
        let mut retired = stale_shards.to_vec();
        retired.extend([
            self.root_dir.join("rulesets/RULE-SET/AdBlock.list"),
            self.root_dir.join("rulesets/RULE-SET/github/AdBlock.list"),
        ]);
        for path in retired {
            let parent = path.parent().ok_or_else(|| {
                format!("retired PROMAX artifact has no parent: {}", path.display())
            })?;
            let relative_parent = parent.strip_prefix(&self.root_dir).map_err(|_| {
                format!(
                    "retired PROMAX artifact escapes repository: {}",
                    path.display()
                )
            })?;
            let mut directory = self.root_dir.clone();
            let mut parent_missing = false;
            for component in relative_parent.components() {
                let std::path::Component::Normal(name) = component else {
                    return Err(format!(
                        "invalid retired PROMAX artifact parent: {}",
                        parent.display()
                    ));
                };
                directory.push(name);
                let metadata = match fs::symlink_metadata(&directory) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        parent_missing = true;
                        break;
                    }
                    Err(error) => {
                        return Err(format!(
                            "cannot inspect retired PROMAX artifact parent {}: {error}",
                            directory.display()
                        ));
                    }
                };
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(format!(
                        "refusing to prune through non-directory or symlink {}",
                        directory.display()
                    ));
                }
            }
            if parent_missing {
                continue;
            }
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "cannot remove retired PROMAX artifact {}: {error}",
                        path.display()
                    ));
                }
            }
        }
        Ok(())
    }

    fn is_known_shard_filename(filename: &str) -> bool {
        let Some(body) = filename
            .strip_prefix("AdBlock_")
            .and_then(|stem| stem.strip_suffix(".list"))
        else {
            return false;
        };
        CATEGORIES.iter().any(|category| {
            if body == *category {
                return true;
            }
            let Some(suffix) = body.strip_prefix(&format!("{category}_")) else {
                return false;
            };
            if !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()) {
                return true;
            }
            ["ip", "other_extra"].iter().any(|kind| {
                suffix == *kind
                    || suffix
                        .strip_prefix(&format!("{kind}_"))
                        .is_some_and(|index| {
                            !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
                        })
            })
        })
    }

    fn prune_stale_split_artifacts(&self, generated_files: &HashSet<String>) {
        let split_dir = self.root_dir.join("modules/source/build/promax_splits");
        if !split_dir.is_dir() {
            return;
        }
        let mut removed = Vec::new();
        if let Ok(entries) = fs::read_dir(split_dir) {
            for entry in entries.map_while(Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".ad-extract.sgmodule")
                    && !generated_files.contains(&name)
                    && fs::remove_file(entry.path()).is_ok()
                {
                    removed.push(name);
                }
            }
        }
        if !removed.is_empty() {
            removed.sort();
            println!(
                "\x1b[0;32m[INFO]\x1b[0m Pruned {} stale PROMAX split artifact(s): {}",
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
        let mut seen_functional_content = HashSet::new();

        for u in remote_urls {
            if let Some(content) = remote_contents.get(&u) {
                let digest = format!("{:x}", md5::compute(content.as_bytes()));
                if !seen_functional_content.insert(digest) {
                    println!("\x1b[0;32m[INFO]\x1b[0m   ⊘ Duplicate functional content: {u}");
                    continue;
                }
                let parts: Vec<&str> = u.split('/').collect();
                println!(
                    "\x1b[0;32m[INFO]\x1b[0m   + Functional remote: {}",
                    parts.last().unwrap_or(&u.as_str())
                );
                let prepared = super::functional::prepare_functional_source(&u, content);
                self.extract_from_text(&prepared, "REJECT", "Local", true, false, &u);
            } else {
                println!(
                    "\x1b[0;33m[WARN]\x1b[0m   ⚠ Functional remote fetch failed: {}",
                    u
                );
            }
        }

        // Current official app modules own their scripts; do not re-add stale aggregate copies.
        let official_app_scripts: HashSet<&str> =
            super::script_resources::required_resources(&self.sections["Script"].join("\n"))
                .into_iter()
                .filter(|url| url.starts_with("https://raw.githubusercontent.com/app2smile/"))
                .collect();
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

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let digest = format!("{:x}", md5::compute(content.as_bytes()));
            if !seen_functional_content.insert(digest) {
                println!("\x1b[0;32m[INFO]\x1b[0m   ⊘ Duplicate functional content: {rel}");
                continue;
            }
            let content = content
                .lines()
                .filter(|line| {
                    super::script_resources::script_resource_url(line)
                        .is_none_or(|url| !official_app_scripts.contains(url))
                })
                .collect::<Vec<_>>()
                .join("\n");
            if ingest_mode == "split" {
                if let Some(filtered) = self.stage_promax_split_artifact(&path, &content, &rel) {
                    self.extract_from_text(&filtered, "REJECT", "Local", true, false, &rel);
                }
            } else {
                self.extract_from_text(&content, "REJECT", "Local", true, false, &rel);
            }
        }

        for section in [
            "URL Rewrite",
            "Map Local",
            "Script",
            "Body Rewrite",
            "Header Rewrite",
        ] {
            let lines = self.sections.get_mut(section).unwrap();
            *lines = crate::smart_cleanup::dedupe_section_lines_pub(section, lines);
        }
        let mut claimed_matchers = HashSet::new();
        for section in ["Map Local", "Body Rewrite", "URL Rewrite"] {
            self.sections.get_mut(section).unwrap().retain(|line| {
                crate::smart_cleanup::functional_matcher_key_pub(section, line)
                    .is_none_or(|matcher| claimed_matchers.insert(matcher))
            });
        }
        self.retain_referenced_mitm_hosts();
        self.optimize_runtime_sections();

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
            let diff = after.saturating_sub(before);
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

    fn split_candidate_rejection(
        &self,
        section: &str,
        line: &str,
    ) -> Option<(String, crate::promax::safety::RiskClass)> {
        if let Some(error) =
            crate::promax::validation::validate_surge_section_line(section, line).first()
        {
            return Some((
                format!("invalid-functional-syntax:{}", error.code),
                crate::promax::safety::RiskClass::Invalid,
            ));
        }

        let decision = if section == "Rule" {
            let parsed = crate::promax::rule::SurgeRule::parse(line).ok()?;
            self.safety.decision(&parsed)
        } else if section == "MITM" {
            return None;
        } else {
            self.safety.functional_decision(line)
        };
        if let crate::promax::safety::SafetyDecision::Quarantine(reason) = decision {
            let risk = if reason.contains("protected") {
                crate::promax::safety::RiskClass::ProtectedService
            } else {
                crate::promax::safety::RiskClass::BroadMedia
            };
            Some((reason.to_string(), risk))
        } else {
            None
        }
    }

    fn filter_split_mitm_hosts(
        &mut self,
        sections: &mut HashMap<String, Vec<String>>,
        source_label: &str,
        source_lines: &HashMap<(String, String), usize>,
    ) {
        let functional_lines: Vec<String> = sections
            .iter()
            .filter(|(section, _)| section.as_str() != "MITM")
            .flat_map(|(_, lines)| lines.iter().cloned())
            .collect();
        let host_patterns =
            crate::promax::safety::SafetyPolicy::functional_host_patterns(&functional_lines);
        let Some(mitm_lines) = sections.get_mut("MITM") else {
            return;
        };

        let mut filtered_lines = Vec::new();
        for line in mitm_lines.iter() {
            let source_line = *source_lines
                .get(&("MITM".to_string(), line.trim().to_string()))
                .unwrap_or(&0);
            let hosts = line
                .split_once('=')
                .map(|(_, hosts)| hosts)
                .unwrap_or("")
                .replace("%APPEND%", "")
                .replace("%INSERT%", "");
            let mut kept_hosts = Vec::new();
            for host in hosts
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
            {
                let rejection = if self.safety.hostname_is_protected(host) {
                    Some((
                        "protected-mitm-host",
                        crate::promax::safety::RiskClass::ProtectedService,
                    ))
                } else if !self
                    .safety
                    .hostname_is_referenced_by_patterns(host, &host_patterns)
                {
                    Some((
                        "unreferenced-mitm-host",
                        crate::promax::safety::RiskClass::BroadMedia,
                    ))
                } else {
                    None
                };
                if let Some((reason, risk)) = rejection {
                    self.quarantine
                        .push(crate::promax::safety::QuarantineRecord {
                            source: source_label.to_string(),
                            line: source_line,
                            candidate: host.to_string(),
                            reason: reason.to_string(),
                            risk,
                        });
                } else {
                    kept_hosts.push(host.to_string());
                }
            }
            if !kept_hosts.is_empty() {
                filtered_lines.push(format!("hostname = %APPEND% {}", kept_hosts.join(", ")));
            }
        }
        *mitm_lines = filtered_lines;
    }

    fn stage_promax_split_artifact(
        &mut self,
        source_path: &Path,
        text: &str,
        source_label: &str,
    ) -> Option<String> {
        let stem = source_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let safe_stem = Regex::new(r"[^\w\u4e00-\u9fff.-]+")
            .unwrap()
            .replace_all(&stem, "_")
            .to_string();
        let out_dir = self.root_dir.join("modules/source/build/promax_splits");
        let out_path = out_dir.join(format!("{}.ad-extract.sgmodule", safe_stem));
        if self
            .split_artifacts
            .iter()
            .any(|artifact| artifact.target == out_path)
        {
            println!(
                "\x1b[0;32m[INFO]\x1b[0m   ↳ split artifact already staged for {}",
                safe_stem
            );
            return None;
        }

        {
            let (mut ad_sections, mut stats) = crate::smart_cleanup::split_module_sections_pub(
                source_path.to_str().unwrap(),
                text,
            );
            let original_kept = *stats.get("kept_lines").unwrap_or(&0);
            let mut source_lines = HashMap::new();
            let mut current_section = String::new();
            for (index, raw_line) in text.lines().enumerate() {
                let stripped = raw_line.trim();
                if stripped.starts_with('[') && stripped.ends_with(']') {
                    current_section = stripped[1..stripped.len() - 1].to_string();
                } else if !stripped.is_empty() && !stripped.starts_with('#') {
                    source_lines
                        .entry((current_section.clone(), stripped.to_string()))
                        .or_insert(index + 1);
                }
            }
            for (section, lines) in &mut ad_sections {
                if section == "Script" {
                    for line in lines.iter_mut() {
                        *line = super::script_resources::normalize_script_line(line);
                    }
                }
                lines.retain(|line| {
                    let rejection = self.split_candidate_rejection(section, line);
                    if let Some((reason, risk)) = rejection {
                        self.quarantine
                            .push(crate::promax::safety::QuarantineRecord {
                                source: source_label.to_string(),
                                line: *source_lines
                                    .get(&(section.clone(), line.trim().to_string()))
                                    .unwrap_or(&0),
                                candidate: line.trim().to_string(),
                                reason,
                                risk,
                            });
                        false
                    } else {
                        true
                    }
                });
                *lines = crate::smart_cleanup::dedupe_section_lines_pub(section, lines);
            }
            self.filter_split_mitm_hosts(&mut ad_sections, source_label, &source_lines);
            ad_sections.retain(|_, lines| !lines.is_empty());
            let filtered_kept: usize = ad_sections.values().map(Vec::len).sum();
            let filtered_out = original_kept.saturating_sub(filtered_kept);
            stats.insert("kept_lines".to_string(), filtered_kept);
            *stats.entry("skipped_lines".to_string()).or_insert(0) += filtered_out;
            let kept = *stats.get("kept_lines").unwrap_or(&0);
            let total = *stats.get("total_lines").unwrap_or(&0);
            if kept == 0 {
                return None;
            }
            let _ = fs::create_dir_all(&out_dir);

            let note = format!(
                "从混合模块拆出的去广告行 {}/{}；解锁/增强仍留在原模块",
                kept, total
            );

            let formatted =
                crate::smart_cleanup::format_split_module_pub(source_label, &ad_sections, &note);
            let rel = out_path
                .strip_prefix(&self.root_dir)
                .unwrap_or(&out_path)
                .to_string_lossy()
                .to_string();
            self.split_artifacts
                .push(crate::promax::artifact::GeneratedArtifact {
                    target: out_path,
                    content: formatted.clone(),
                    kind: crate::promax::artifact::ArtifactKind::Surge,
                });
            println!(
                "\x1b[0;32m[INFO]\x1b[0m   ↳ split artifact: {} ({}/{} lines)",
                rel, kept, total
            );
            Some(formatted)
        }
    }

    fn load_functional_source_paths(&self) -> (Vec<(PathBuf, &'static str)>, Vec<String>) {
        let mut paths = Vec::new();
        let mut remote_urls = Vec::new();
        let mut seen = HashSet::new();

        let func_sources_file = self
            .root_dir
            .join("rulesets/Sources/Links/AdBlock_functional_sources.list");
        if func_sources_file.exists()
            && let Ok(file) = File::open(&func_sources_file)
        {
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

        // Local local modules dir
        let local_dir = self.root_dir.join("modules/source/local");
        if local_dir.is_dir()
            && let Ok(entries) = fs::read_dir(local_dir)
        {
            let mut files: Vec<PathBuf> = entries.map_while(Result::ok).map(|e| e.path()).collect();
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

        // LocalModules dir
        let local_modules_dir = self.root_dir.join("rulesets/Sources/LocalModules");
        if local_modules_dir.is_dir()
            && let Ok(entries) = fs::read_dir(local_modules_dir)
        {
            let mut files: Vec<PathBuf> = entries.map_while(Result::ok).map(|e| e.path()).collect();
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
        let now = crate::publication_now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let surge_promax_cdn = "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule";
        let surge_promax_github = "modules/surge/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule";

        let sr_promax_cdn = "modules/shadowrocket/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module";
        let sr_promax_github = "modules/shadowrocket/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module";

        let mut modules = BTreeMap::new();
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
            let filename = rs_path.split('/').next_back().unwrap();
            let category = self.category_from_filename(filename);
            let meta = CATEGORY_META.get(category).unwrap();
            let shard_index = SHARD_INDEX_RE
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

        let mut functional_stats = BTreeMap::new();
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
            verified_rolling_feeds: self.verified_rolling_feeds.clone(),
            shards,
        };

        let catalog_path = self.root_dir.join("rulesets/AdBlock/catalog.json");
        let json_str = serde_json::to_string_pretty(&catalog_data).unwrap();

        let readme = render_catalog_readme(&catalog_data);
        let readme_path = self.root_dir.join("rulesets/AdBlock/README.md");
        vec![
            crate::promax::artifact::GeneratedArtifact {
                target: catalog_path,
                content: format!("{}\n", json_str),
                kind: crate::promax::artifact::ArtifactKind::Metadata,
            },
            crate::promax::artifact::GeneratedArtifact {
                target: readme_path,
                content: readme,
                kind: crate::promax::artifact::ArtifactKind::Metadata,
            },
        ]
    }

    fn generate_module(
        &self,
        generated_rulesets: &[String],
        url_source: &str,
    ) -> crate::promax::artifact::GeneratedArtifact {
        let current_date = crate::publication_now().format("%Y-%m-%d").to_string();
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
        if ports_source.exists()
            && let Ok(file) = File::open(&ports_source)
        {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                let stripped = line.trim();
                if !stripped.is_empty() && !stripped.starts_with('#') {
                    rule_lines.push(stripped.to_string());
                }
            }
        }

        rule_lines.extend(vec![
            "# --- SYNCED PORT RULES END ---".to_string(),
            "# HTTPDNS rules are compiled from the maintained AdBlock source manifest.".to_string(),
            "# Split REJECT Rulesets (purpose-grouped; see rulesets/AdBlock/README.md)".to_string(),
        ]);

        let mut shards_by_category: HashMap<String, Vec<String>> = HashMap::new();
        for rs_path in generated_rulesets {
            let cat = self.category_from_filename(rs_path.split('/').next_back().unwrap());
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
                    let name = p.split('/').next_back().unwrap();
                    let shard_index = SHARD_INDEX_RE
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

        // Every upstream, including DROP/NO-DROP lists, must pass the same
        // whitelist and safety checks before its rules reach the module.

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
                if let Some(line) = self.attach_policy(&raw, policy)
                    && complex_seen.insert(line.clone())
                {
                    rule_lines.push(line);
                }
            }
        }
        let rule_lines = crate::smart_cleanup::dedupe_section_lines_pub("Rule", &rule_lines);

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
                    name: sec_name.to_string(),
                    lines: deduped.clone(),
                });
                func_counts.push(format!("{}={}", sec_name, deduped.len()));
            }
        }

        if !self.mitm_hosts.is_empty() {
            let mut hosts: Vec<String> = self.mitm_hosts.iter().cloned().collect();
            hosts.sort();
            let mitm_line = format!("hostname = %APPEND% {}", hosts.join(", "));
            functional_sections.push(crate::smart_cleanup::ModuleSectionPub {
                name: "MITM".to_string(),
                lines: vec![mitm_line],
            });
            func_counts.push(format!("MITM={}", self.mitm_hosts.len()));
        }

        let mut all_sections = vec![crate::smart_cleanup::ModuleSectionPub {
            name: "Rule".to_string(),
            lines: rule_lines,
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
            crate::publication_now()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        );

        let mut header_lines = crate::smart_cleanup::format_header_pub(&header_meta);
        header_lines.push(
            "# app2smile/rules + AdGuard CNAME: MIT; notices: rulesets/Sources/licenses/README.md"
                .to_string(),
        );
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

    fn attach_policy(&self, rule: &str, policy: &str) -> Option<String> {
        let mut parsed = crate::promax::rule::SurgeRule::parse(rule).ok()?;
        parsed.policy = Some(policy.to_ascii_uppercase());
        // Modules precede consumer routing rules: never resolve a domain just
        // to test an IP block supplied by PROMAX (including future upstreams).
        parsed.prevent_dns_resolution();
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
            let cat = e
                .category
                .as_deref()
                .unwrap_or_else(|| self.determine_category(&e.source));
            *priority_map.get(cat).unwrap_or(&99)
        });

        let remote_contents = match self.download_remote_contents(&source_entries) {
            Ok(contents) => contents,
            Err(failures) => {
                for failure in &failures {
                    println!(
                        "\x1b[0;31m[ERROR]\x1b[0m PROMAX source download failed: {} ({})",
                        crate::redact_url(&failure.url),
                        failure.error
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
        if local_dir.is_dir()
            && let Ok(entries) = fs::read_dir(&local_dir)
        {
            let mut files: Vec<PathBuf> = entries.map_while(Result::ok).map(|e| e.path()).collect();
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

        println!("\x1b[0;32m[INFO]\x1b[0m Integrating App AdBlock Rules (LocalModules)");
        let local_modules_dir = self.root_dir.join("rulesets/Sources/LocalModules");
        if local_modules_dir.is_dir()
            && let Ok(entries) = fs::read_dir(&local_modules_dir)
        {
            let mut files: Vec<PathBuf> = entries.map_while(Result::ok).map(|e| e.path()).collect();
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
            let (generated_rulesets, mut artifacts) = match self.generate_ruleset() {
                Ok(generated) => generated,
                Err(error) => {
                    eprintln!("[ERROR] Refusing PROMAX generation: {error}");
                    return false;
                }
            };
            if let Err(error) = crate::promax::artifact::validate_coverage_floor_with_rolling(
                &self.root_dir,
                &artifacts,
                &self.verified_rolling_feeds,
            ) {
                println!("\x1b[0;31m[ERROR]\x1b[0m Refusing PROMAX coverage regression: {error}");
                return false;
            }
            artifacts.extend(self.generate_catalog_artifacts(&generated_rulesets, &artifacts));
            let generated_splits: HashSet<String> = self
                .split_artifacts
                .iter()
                .filter_map(|artifact| {
                    artifact
                        .target
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .collect();
            artifacts.append(&mut self.split_artifacts);

            println!(
                "\x1b[0;32m[INFO]\x1b[0m Rendering PROMAX Surge and Shadowrocket artifacts..."
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
                shadowrocket_cdn,
                shadowrocket_github,
            ]);
            let stale_shards = match self.stale_ruleset_shard_paths(&generated_rulesets) {
                Ok(paths) => paths,
                Err(error) => {
                    println!("\x1b[0;31m[ERROR]\x1b[0m Refusing PROMAX publication: {error}");
                    return false;
                }
            };
            if let Err(error) = crate::promax::artifact::publish_artifacts(
                &self.root_dir,
                &artifacts,
                &self.quarantine,
            ) {
                println!("\x1b[0;31m[ERROR]\x1b[0m Refusing PROMAX publication: {error}");
                return false;
            }
            if let Err(error) = self.prune_legacy_and_stale_rulesets(&stale_shards) {
                println!(
                    "\x1b[0;31m[ERROR]\x1b[0m PROMAX publication completed but retired artifact cleanup failed: {error}"
                );
                return false;
            }
            self.prune_stale_split_artifacts(&generated_splits);
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
                if p.exists()
                    && let Ok(rel) = p.strip_prefix(&self.root_dir)
                    && let Ok(bytes) = fs::read(&p)
                {
                    let digest = md5::compute(bytes);
                    input_hashes.insert(rel.to_string_lossy().to_string(), format!("{:x}", digest));
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

fn rewrite_module_rule_sets(
    content: &str,
    old_categories: &HashMap<String, String>,
    new_paths: &HashMap<String, Vec<String>>,
) -> Result<String, String> {
    let mut output = String::with_capacity(content.len());
    let mut expanded = HashSet::new();
    for line in content.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let ending = &line[body.len()..];
        let trimmed = body.trim_start();
        if !trimmed.starts_with("RULE-SET,") {
            output.push_str(line);
            continue;
        }
        let matching = old_categories
            .iter()
            .filter(|(path, _)| body.contains(path.as_str()))
            .max_by_key(|(path, _)| path.len());
        let Some((old_path, category)) = matching else {
            if body.contains("rulesets/AdBlock/") {
                return Err(format!(
                    "module contains an unlisted PROMAX shard reference: {body}"
                ));
            }
            output.push_str(line);
            continue;
        };
        let start = body.find(old_path).expect("matched path is present");
        let end = start + old_path.len();
        let prefix = &body[..start];
        let suffix = &body[end..];
        let key = format!("{category}\0{prefix}\0{suffix}");
        if !expanded.insert(key) {
            continue;
        }
        let paths = new_paths
            .get(category)
            .ok_or_else(|| format!("no migrated PROMAX shards for category {category}"))?;
        for path in paths {
            output.push_str(prefix);
            output.push_str(path);
            output.push_str(suffix);
            output.push_str(ending);
        }
    }
    Ok(output)
}

fn ensure_migration_target_inside_root(root: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("migration target has no parent: {}", target.display()))?;
    let resolved = parent.canonicalize().map_err(|error| {
        format!(
            "cannot resolve migration target parent {}: {error}",
            parent.display()
        )
    })?;
    if !resolved.starts_with(root) {
        return Err(format!(
            "migration target escapes root: {}",
            target.display()
        ));
    }
    Ok(())
}

/// Refreshes only the published README from the local catalog and validated shards, without network access.
pub fn refresh_published_index(root: &Path) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("cannot resolve index root: {error}"))?;
    let catalog_path = root.join("rulesets/AdBlock/catalog.json");
    ensure_migration_target_inside_root(&root, &catalog_path)?;
    let content = fs::read_to_string(&catalog_path)
        .map_err(|error| format!("cannot read PROMAX catalog: {error}"))?;
    let catalog: CatalogJson = serde_json::from_str(&content)
        .map_err(|error| format!("cannot parse PROMAX catalog: {error}"))?;
    let module_keys = [
        "surge_promax_cdn",
        "surge_promax_github",
        "shadowrocket_promax_cdn",
        "shadowrocket_promax_github",
    ];
    if catalog.shards.is_empty()
        || catalog.modules.len() != module_keys.len()
        || module_keys
            .iter()
            .any(|key| !catalog.modules.contains_key(*key))
    {
        return Err("PROMAX catalog is missing published shards or modules".to_string());
    }
    let mut files = HashSet::new();
    for shard in &catalog.shards {
        let relative = Path::new(&shard.file);
        if relative.parent() != Some(Path::new("rulesets/AdBlock"))
            || !relative
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(AdBlockManager::is_known_shard_filename)
            || !CATEGORIES.contains(&shard.category.as_str())
            || !files.insert(&shard.file)
        {
            return Err(format!("invalid PROMAX catalog shard: {}", shard.file));
        }
        let target = root.join(relative);
        ensure_migration_target_inside_root(&root, &target)?;
        let content = fs::read_to_string(&target)
            .map_err(|error| format!("cannot read published shard {}: {error}", shard.file))?;
        let count = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with(['#', ';', '/']))
            .collect::<HashSet<_>>()
            .len();
        if count == 0
            || count != shard.rule_count
            || !crate::promax::validation::validate_external_ruleset(&content).is_empty()
        {
            return Err(format!("published shard {} failed validation", shard.file));
        }
    }
    let target = root.join("rulesets/AdBlock/README.md");
    ensure_migration_target_inside_root(&root, &target)?;
    crate::promax::artifact::publish_generated_artifacts(
        &root,
        &[crate::promax::artifact::GeneratedArtifact {
            target,
            content: render_catalog_readme(&catalog),
            kind: crate::promax::artifact::ArtifactKind::Metadata,
        }],
    )
}

fn update_migration_readme(content: &str, shards: &[serde_json::Value]) -> Result<String, String> {
    const ENGLISH_HEADER: &str = "| Category | File | Rules | CDN | GitHub |";
    let (header, header_start) = [ENGLISH_HEADER, "| 用途 | 文件 | 规则数 | CDN | GitHub |"]
        .into_iter()
        .find_map(|header| content.find(header).map(|start| (header, start)))
        .ok_or_else(|| "PROMAX README is missing the current-shards table".to_string())?;
    let mut body_start = header_start + header.len();
    if content[body_start..].starts_with("\r\n") {
        body_start += 2;
    } else if content[body_start..].starts_with('\n') {
        body_start += 1;
    }
    let next_start = content[body_start..]
        .find("\n## ")
        .map(|offset| body_start + offset)
        .ok_or_else(|| "PROMAX README is missing the following section marker".to_string())?;
    let mut rows = "| --- | --- | --- | --- | --- |\n".to_string();
    for shard in shards {
        let label = shard[if header == ENGLISH_HEADER {
            "category"
        } else {
            "label_zh"
        }]
        .as_str()
        .unwrap_or_default();
        let file = shard["file"].as_str().unwrap_or_default();
        let count = shard["rule_count"].as_u64().unwrap_or_default();
        let cdn = shard["cdn_url"].as_str().unwrap_or_default();
        let github = shard["github_url"].as_str().unwrap_or_default();
        rows.push_str(&format!(
            "| {label} | `{file}` | {count} | [CDN]({cdn}) | [GitHub]({github}) |\n"
        ));
    }
    let mut updated = String::with_capacity(content.len() + rows.len());
    updated.push_str(&content[..body_start]);
    updated.push_str(&rows);
    updated.push_str(&content[next_start..]);
    Ok(updated)
}

/// Mechanically split the published, validated PROMAX shards without fetching sources.
pub fn migrate_published_rule_layout(root: &Path) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("cannot resolve migration root: {error}"))?;
    let catalog_path = root.join("rulesets/AdBlock/catalog.json");
    ensure_migration_target_inside_root(&root, &catalog_path)?;
    let catalog_content = fs::read_to_string(&catalog_path)
        .map_err(|error| format!("cannot read PROMAX catalog: {error}"))?;
    let mut catalog: serde_json::Value = serde_json::from_str(&catalog_content)
        .map_err(|error| format!("cannot parse PROMAX catalog: {error}"))?;
    let old_shards = catalog["shards"]
        .as_array()
        .ok_or_else(|| "PROMAX catalog is missing its shard list".to_string())?;
    if old_shards.is_empty() {
        return Err("PROMAX catalog has no published shards to migrate".to_string());
    }

    let mut manager = AdBlockManager::new(root.clone());
    let mut old_categories = HashMap::new();
    let mut source_rules = HashMap::<String, HashSet<String>>::new();
    let mut source_counts = HashMap::<String, usize>::new();
    for shard in old_shards {
        let file = shard["file"]
            .as_str()
            .ok_or_else(|| "PROMAX catalog contains a shard without a file path".to_string())?;
        let category = shard["category"]
            .as_str()
            .filter(|category| CATEGORIES.contains(category))
            .ok_or_else(|| format!("PROMAX catalog contains an unknown shard category: {file}"))?;
        let name = file
            .strip_prefix("rulesets/AdBlock/")
            .ok_or_else(|| format!("PROMAX shard is outside rulesets/AdBlock: {file}"))?;
        if name.contains('/')
            || name.contains('\\')
            || !AdBlockManager::is_known_shard_filename(name)
            || Path::new(file).parent() != Some(Path::new("rulesets/AdBlock"))
            || manager.category_from_filename(name) != category
            || old_categories
                .insert(file.to_string(), category.to_string())
                .is_some()
        {
            return Err(format!(
                "invalid or duplicate PROMAX catalog shard path: {file}"
            ));
        }
        let shard_path = root.join(file);
        ensure_migration_target_inside_root(&root, &shard_path)?;
        let content = fs::read_to_string(&shard_path)
            .map_err(|error| format!("cannot read published shard {file}: {error}"))?;
        let validation = crate::promax::validation::validate_external_ruleset(&content);
        if !validation.is_empty() {
            return Err(format!("published shard {file} no longer validates"));
        }
        let expected_count = shard["rule_count"]
            .as_u64()
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| format!("PROMAX catalog has an invalid rule count for {file}"))?;
        let mut count = 0usize;
        for line in content.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            crate::promax::rule::SurgeRule::parse(line)
                .map_err(|error| format!("invalid rule in {file}: {error}"))?;
            count = count.saturating_add(1);
            source_rules
                .entry(category.to_string())
                .or_default()
                .insert(line.to_string());
            manager
                .rules
                .get_mut("REJECT")
                .and_then(|policies| policies.get_mut(category))
                .ok_or_else(|| format!("PROMAX category is unavailable: {category}"))?
                .insert(line.to_string());
        }
        if count != expected_count {
            return Err(format!(
                "published shard {file} has {count} rules but catalog records {expected_count}"
            ));
        }
        *source_counts.entry(category.to_string()).or_default() += count;
    }

    let (generated_files, mut artifacts) = manager.generate_ruleset()?;
    let mut new_paths = HashMap::<String, Vec<String>>::new();
    let mut migrated_rules = HashMap::<String, HashSet<String>>::new();
    let mut migrated_counts = HashMap::<String, usize>::new();
    let mut new_shards = Vec::new();
    for artifact in &artifacts {
        let category = artifact
            .content
            .lines()
            .find_map(|line| line.strip_prefix("# Category-Key: "))
            .ok_or_else(|| {
                format!(
                    "generated shard has no category key: {}",
                    artifact.target.display()
                )
            })?;
        let category_meta = CATEGORY_META
            .get(category)
            .ok_or_else(|| format!("generated shard has unknown category {category}"))?;
        let relative = artifact
            .target
            .strip_prefix(&root)
            .map_err(|_| {
                format!(
                    "generated shard escapes migration root: {}",
                    artifact.target.display()
                )
            })?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let count = artifact
            .content
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
            .count();
        let rules = migrated_rules.entry(category.to_string()).or_default();
        for line in artifact.content.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            rules.insert(line.to_string());
        }
        *migrated_counts.entry(category.to_string()).or_default() += count;
        new_paths
            .entry(category.to_string())
            .or_default()
            .push(relative.clone());
        let shard_index = SHARD_INDEX_RE
            .captures(&relative)
            .and_then(|captures| captures[1].parse::<usize>().ok())
            .unwrap_or(1);
        let encoded =
            percent_encoding::utf8_percent_encode(&relative, percent_encoding::NON_ALPHANUMERIC);
        new_shards.push(serde_json::json!({
            "file": relative,
            "category": category,
            "label_zh": category_meta.label_zh,
            "purpose": category_meta.desc,
            "shard_index": shard_index,
            "rule_count": count,
            "cdn_url": format!("{}{}", CDN_BASE_URL, encoded),
            "github_url": format!("{}{}", GITHUB_RAW_URL, encoded),
        }));
    }
    if migrated_rules != source_rules || migrated_counts != source_counts {
        return Err("PROMAX shard migration changed a category rule set or count".to_string());
    }

    let readme_path = root.join("rulesets/AdBlock/README.md");
    if readme_path.is_file() {
        ensure_migration_target_inside_root(&root, &readme_path)?;
        let readme = fs::read_to_string(&readme_path)
            .map_err(|error| format!("cannot read PROMAX README: {error}"))?;
        artifacts.push(crate::promax::artifact::GeneratedArtifact {
            target: readme_path,
            content: update_migration_readme(&readme, &new_shards)?,
            kind: crate::promax::artifact::ArtifactKind::Metadata,
        });
    }

    let modules = catalog["modules"]
        .as_object()
        .ok_or_else(|| "PROMAX catalog is missing its module map".to_string())?;
    let mut module_paths = modules
        .values()
        .map(|module| {
            module["path"]
                .as_str()
                .ok_or_else(|| "PROMAX catalog contains a module without a path".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    module_paths.sort_unstable();
    if module_paths.is_empty() {
        return Err("PROMAX catalog has no modules to migrate".to_string());
    }
    for relative in module_paths {
        if relative.contains('\\')
            || Path::new(relative).is_absolute()
            || Path::new(relative)
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || !relative.starts_with("modules/")
        {
            return Err(format!("invalid PROMAX module path in catalog: {relative}"));
        }
        let target = root.join(relative);
        ensure_migration_target_inside_root(&root, &target)?;
        let content = fs::read_to_string(&target)
            .map_err(|error| format!("cannot read PROMAX module {relative}: {error}"))?;
        let migrated = rewrite_module_rule_sets(&content, &old_categories, &new_paths)?;
        let kind = match target.extension().and_then(|extension| extension.to_str()) {
            Some("sgmodule") => crate::promax::artifact::ArtifactKind::Surge,
            Some("module") => crate::promax::artifact::ArtifactKind::Shadowrocket,
            _ => return Err(format!("unsupported PROMAX module artifact: {relative}")),
        };
        artifacts.push(crate::promax::artifact::GeneratedArtifact {
            target,
            content: migrated,
            kind,
        });
    }

    catalog["shards"] = serde_json::Value::Array(new_shards);
    artifacts.push(crate::promax::artifact::GeneratedArtifact {
        target: catalog_path,
        content: format!(
            "{}\n",
            serde_json::to_string_pretty(&catalog).map_err(|error| error.to_string())?
        ),
        kind: crate::promax::artifact::ArtifactKind::Metadata,
    });
    let stale_shards = manager.stale_ruleset_shard_paths(&generated_files)?;
    crate::promax::artifact::publish_generated_artifacts(&root, &artifacts)?;
    manager.prune_legacy_and_stale_rulesets(&stale_shards)?;
    Ok(())
}

#[cfg(test)]
mod tests;
