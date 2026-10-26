//! Rust-owned refresh for the non-PROMAX ruleset manifests.

use crate::promax::safety::IpNetwork;
use crate::promax::source::{DownloadConfig, Downloader};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const NSFW_PROTECTED_CATEGORY_FILES: &[&str] = &[
    "AI.list",
    "Apple.list",
    "AppleNews.list",
    "Bilibili.list",
    "Binance.list",
    "Cloudflare.list",
    "Direct.list",
    "Gaming.list",
    "Gaming_Download.list",
    "Gaming_Play.list",
    "Gaming_Web.list",
    "GitHub.list",
    "Google.list",
    "Microsoft.list",
    "Netflix.list",
    "PayPal.list",
    "SocialMedia.list",
    "Spotify.list",
    "StreamEU.list",
    "StreamHK.list",
    "StreamJP.list",
    "StreamKR.list",
    "StreamTW.list",
    "StreamUS.list",
];

const NSFW_PROTECTED_DOMAINS: &[&str] = &[
    "10bet.com",
    "1xbet.cm",
    "1xbet.co.ke",
    "9anime.to",
    "booth.pm",
    "cehd.uchicago.edu",
    "cdn.discordapp.com",
    "cdn.statically.io",
    "chicagoreader.com",
    "crunchyroll.com",
    "deviantart.com",
    "df-bet.com",
    "fanbox.cc",
    "fantia.jp",
    "fc2.com",
    "fhm.com",
    "hidive.com",
    "images.pexels.com",
    "images.weserv.nl",
    "media.discordapp.net",
    "patreon.com",
    "pinterest.com",
    "pixiv.net",
    "retrocrush.tv",
    "tubi.tv",
    "tumblr.com",
    "uploads-ssl.webflow.com",
    "wortfm.org",
];

#[derive(Debug)]
struct Manifest {
    name: String,
    local_paths: Vec<PathBuf>,
    remote_urls: Vec<String>,
}

fn discover_manifests(root: &Path) -> Result<Vec<Manifest>, String> {
    let directory = root.join("rulesets/Sources/Links");
    let mut paths: Vec<PathBuf> = fs::read_dir(&directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_sources.list") && !name.starts_with("AdBlock"))
        })
        .collect();
    paths.sort();

    let mut manifests = Vec::new();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix("_sources.list"))
            .ok_or_else(|| format!("invalid ruleset manifest name: {}", path.display()))?
            .to_string();
        let content = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let mut local_paths = BTreeSet::new();
        let mut remote_urls = BTreeSet::new();
        for raw_line in content.lines() {
            if raw_line.trim().starts_with('#') {
                continue;
            }
            let line = crate::strip_inline_comment(raw_line);
            let source = line.split('|').next().unwrap_or("").trim();
            if source.is_empty() {
                continue;
            }
            if source.starts_with("https://") || source.starts_with("http://") {
                remote_urls.insert(source.to_string());
            } else {
                let resolved = path
                    .parent()
                    .unwrap_or(&directory)
                    .join(source)
                    .components()
                    .collect::<PathBuf>();
                local_paths.insert(resolved);
            }
        }
        manifests.push(Manifest {
            name,
            local_paths: local_paths.into_iter().collect(),
            remote_urls: remote_urls.into_iter().collect(),
        });
    }
    Ok(manifests)
}

fn downloader() -> Downloader {
    Downloader::new(DownloadConfig {
        attempts: 3,
        timeout: Duration::from_secs(75),
        batch_timeout: Duration::from_secs(10 * 60),
        backoff: Duration::from_secs(1),
        concurrency: 4,
        max_bytes: 32 * 1024 * 1024,
        max_total_bytes: 256 * 1024 * 1024,
    })
}

fn remote_content(remote_urls: &[String], downloads: &HashMap<String, String>) -> String {
    let (source_count, content_bytes) = remote_urls
        .iter()
        .filter_map(|url| downloads.get(url))
        .fold((0usize, 0usize), |(count, bytes), content| {
            (count + 1, bytes.saturating_add(content.len()))
        });
    let capacity = content_bytes.saturating_add(source_count.saturating_sub(1));
    let mut output = String::with_capacity(capacity);
    let mut first_source = true;
    for url in remote_urls {
        if let Some(content) = downloads.get(url) {
            if !first_source {
                output.push('\n');
            }
            output.push_str(content);
            first_source = false;
        }
    }
    output
}

fn manifest_header_policy(name: &str) -> &'static str {
    match name {
        "Direct" => "DIRECT",
        "Apple" => "DIRECT",
        "NSFW" => "REJECT",
        _ => "PROXY",
    }
}

fn process_manifest(
    root: &Path,
    manifest: &Manifest,
    downloads: &HashMap<String, String>,
) -> Result<(), String> {
    let missing_local: Vec<String> = manifest
        .local_paths
        .iter()
        .filter(|path| !path.is_file())
        .map(|path| path.display().to_string())
        .collect();
    if !missing_local.is_empty() {
        return Err(format!(
            "missing local source(s): {}",
            missing_local.join(", ")
        ));
    }
    let missing_remote: Vec<&str> = manifest
        .remote_urls
        .iter()
        .filter(|url| !downloads.contains_key(*url))
        .map(String::as_str)
        .collect();
    if !missing_remote.is_empty() {
        return Err(format!(
            "missing remote source(s): {}",
            missing_remote.join(", ")
        ));
    }

    let target = root.join("rulesets/RULE-SET");
    fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    let remote_content = remote_content(&manifest.remote_urls, downloads);
    let policy = manifest_header_policy(&manifest.name);
    crate::process_ruleset(crate::RulesetInput {
        name: &manifest.name,
        target_dir: &target,
        conflict_domains: &[],
        skip_conflict: false,
        policy,
        node: "Any",
        description: "Rust-merged canonical ruleset",
        local_paths: &manifest.local_paths,
        remote_content: &remote_content,
    })
    .map_err(|error| format!("Rust ruleset merge failed for {}: {error}", manifest.name))
}

pub fn run_general_updates(root: &Path) -> Vec<String> {
    let manifests = match discover_manifests(root) {
        Ok(manifests) => manifests,
        Err(error) => return vec![error],
    };
    let urls: BTreeSet<String> = manifests
        .iter()
        .flat_map(|manifest| manifest.remote_urls.iter().cloned())
        .collect();
    println!(
        "[INFO] refreshing {} general ruleset(s) from {} unique remote source(s)",
        manifests.len(),
        urls.len()
    );
    let batch = downloader().download_many(urls);
    for failure in &batch.failures {
        eprintln!(
            "[WARN] general source unavailable: {} ({})",
            failure.url, failure.error
        );
    }

    let mut failures = Vec::new();
    for manifest in &manifests {
        match process_manifest(root, manifest, &batch.contents) {
            Ok(()) => println!("[SUCCESS] general ruleset refreshed: {}", manifest.name),
            Err(error) => {
                eprintln!(
                    "[WARN] general ruleset kept at last validated version: {} ({error})",
                    manifest.name
                );
                failures.push(manifest.name.clone());
            }
        }
    }
    failures
}

fn read_rules(path: &Path) -> Result<BTreeSet<String>, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect())
}

fn write_rules(
    path: &Path,
    name: &str,
    policy: &str,
    rules: &BTreeSet<String>,
) -> Result<(), String> {
    let description = match name {
        "Gaming_Download" => {
            "Game packages and patches; place before Gaming_Web and generic routing"
        }
        "Gaming_Play" => {
            "Game networking and matchmaking; place before Gaming_Web and generic routing"
        }
        "Gaming_Web" => {
            "Stores, accounts, communities and shared-platform fallback; not a gameplay-only list"
        }
        _ => "Rust-merged canonical ruleset",
    };
    let mut content = crate::generate_header(name, policy, "Any", description, rules.len());
    for rule in rules {
        content.push_str(rule);
        content.push('\n');
    }
    crate::safe_write_file_internal(path, &content, true)
        .then_some(())
        .ok_or_else(|| format!("failed to publish {}", path.display()))
}

/// Split the compatibility union using explicit endpoints, retaining ambiguous
/// publisher domains only in the last, shared-platform fallback group.
pub fn split_gaming_rules(root: &Path) -> Result<(), String> {
    let directory = root.join("rulesets/RULE-SET");
    let legacy = read_rules(&directory.join("Gaming.list"))?;
    let load = |name: &str| -> Result<BTreeSet<String>, String> {
        read_rules(&root.join(format!("rulesets/Sources/custom/{name}_manual.list")))?
            .into_iter()
            .map(|rule| {
                crate::normalize_rule(&rule)
                    .ok_or_else(|| format!("invalid {name} classification rule: {rule}"))
            })
            .collect()
    };
    let mut download = if directory.join("Gaming_Download.list").is_file() {
        read_rules(&directory.join("Gaming_Download.list"))?
    } else {
        BTreeSet::new()
    };
    download.extend(load("Gaming_Download")?);
    let mut play = load("Gaming_Play")?;
    let download_boundary = RuleBoundary::from_rules(&download);
    let play_boundary = RuleBoundary::from_rules(&play);
    if play.iter().any(|rule| download_boundary.intersects(rule)) {
        return Err("Gaming Download and Play classification endpoints overlap".to_string());
    }
    let mut web = BTreeSet::new();
    for rule in legacy {
        if download_boundary.covers(&rule) {
            download.insert(rule);
        } else if play_boundary.covers(&rule) {
            play.insert(rule);
        } else if rule.starts_with("DOMAIN,") || rule.starts_with("DOMAIN-SUFFIX,") {
            // Shared Microsoft sign-in also serves non-game apps; keep its normal routing.
            if !matches!(
                rule.split(',').nth(1),
                Some("login.live.com" | "logincdn.msauth.net")
            ) {
                web.insert(rule);
            }
        }
    }
    let mirror = directory.join("github");
    fs::create_dir_all(&mirror).map_err(|error| error.to_string())?;
    for (name, rules) in [
        ("Gaming_Download", &download),
        ("Gaming_Play", &play),
        ("Gaming_Web", &web),
    ] {
        write_rules(
            &directory.join(format!("{name}.list")),
            name,
            "USER-DEFINED",
            rules,
        )?;
        write_rules(
            &mirror.join(format!("{name}.list")),
            name,
            "USER-DEFINED",
            rules,
        )?;
    }
    Ok(())
}

#[derive(Default)]
struct RuleBoundary {
    domains: HashSet<String>,
    suffixes: HashSet<String>,
    other: HashSet<String>,
    reversed_hosts: Vec<String>,
    ip_networks: Vec<IpNetwork>,
}

fn parse_ip_rule(rule: &str) -> Option<IpNetwork> {
    let mut fields = rule.split(',');
    let kind = fields.next()?;
    let value = fields.next()?;
    let expected_ipv6 = if kind.eq_ignore_ascii_case("IP-CIDR") {
        false
    } else if kind.eq_ignore_ascii_case("IP-CIDR6") {
        true
    } else {
        return None;
    };
    let address = value.split('/').next()?;
    (address.contains(':') == expected_ipv6)
        .then(|| IpNetwork::parse(value))
        .flatten()
}

fn reverse_domain_labels(host: &str) -> String {
    let mut reversed = String::with_capacity(host.len());
    let mut first = true;
    for label in host.rsplit('.') {
        if !first {
            reversed.push('.');
        }
        first = false;
        reversed.push_str(label);
    }
    reversed
}

impl RuleBoundary {
    fn from_rules<'a>(rules: impl IntoIterator<Item = &'a String>) -> Self {
        let mut boundary = Self::default();
        for rule in rules {
            boundary.insert(rule);
        }
        boundary.finish();
        boundary
    }

    fn insert(&mut self, rule: &str) {
        let mut fields = rule.split(',');
        match (fields.next(), fields.next()) {
            (Some(kind), Some(value)) if kind.eq_ignore_ascii_case("DOMAIN") => {
                self.domains.insert(value.to_ascii_lowercase());
            }
            (Some(kind), Some(value)) if kind.eq_ignore_ascii_case("DOMAIN-SUFFIX") => {
                self.suffixes.insert(value.to_ascii_lowercase());
            }
            (Some(kind), Some(_))
                if kind.eq_ignore_ascii_case("IP-CIDR")
                    || kind.eq_ignore_ascii_case("IP-CIDR6") =>
            {
                if let Some(network) = parse_ip_rule(rule) {
                    self.ip_networks.push(network);
                } else {
                    self.other.insert(rule.to_string());
                }
            }
            _ => {
                self.other.insert(rule.to_string());
            }
        }
    }

    fn finish(&mut self) {
        let capacity = self.domains.len().saturating_add(self.suffixes.len());
        let mut reversed_hosts = Vec::with_capacity(capacity);
        reversed_hosts.extend(
            self.domains
                .iter()
                .chain(&self.suffixes)
                .map(|host| reverse_domain_labels(host)),
        );
        self.reversed_hosts = reversed_hosts;
        self.reversed_hosts.sort();
        self.reversed_hosts.dedup();
    }

    fn suffix_ancestor(&self, host: &str) -> bool {
        let mut candidate = host;
        loop {
            if self.suffixes.contains(candidate) {
                return true;
            }
            let Some((_, parent)) = candidate.split_once('.') else {
                return false;
            };
            candidate = parent;
        }
    }

    fn has_descendant(&self, suffix: &str) -> bool {
        let mut prefix = reverse_domain_labels(suffix);
        prefix.push('.');
        let index = self.reversed_hosts.partition_point(|host| host < &prefix);
        self.reversed_hosts
            .get(index)
            .is_some_and(|host| host.starts_with(&prefix))
    }

    fn intersects(&self, rule: &str) -> bool {
        let mut fields = rule.split(',');
        match (fields.next(), fields.next()) {
            (Some(kind), Some(value)) if kind.eq_ignore_ascii_case("DOMAIN") => {
                let value = value.to_ascii_lowercase();
                self.domains.contains(&value) || self.suffix_ancestor(&value)
            }
            (Some(kind), Some(value)) if kind.eq_ignore_ascii_case("DOMAIN-SUFFIX") => {
                let value = value.to_ascii_lowercase();
                self.suffix_ancestor(&value) || self.has_descendant(&value)
            }
            (Some(kind), Some(_))
                if kind.eq_ignore_ascii_case("IP-CIDR")
                    || kind.eq_ignore_ascii_case("IP-CIDR6") =>
            {
                parse_ip_rule(rule).map_or_else(
                    || self.other.contains(rule),
                    |network| {
                        self.ip_networks
                            .iter()
                            .any(|boundary| boundary.overlaps(network))
                    },
                )
            }
            _ => self.other.contains(rule),
        }
    }

    /// Returns true only when this boundary covers the rule's own match target.
    ///
    /// This is intentionally directional. A broad DIRECT suffix may contain a
    /// more-specific advertising child without being contaminated itself (for
    /// example, `example.com` and `ads.example.com`). Removing the broad suffix
    /// would send all normal service traffic to FINAL merely to eliminate that
    /// valid parent/child relationship.
    fn covers(&self, rule: &str) -> bool {
        let mut fields = rule.split(',');
        match (fields.next(), fields.next()) {
            (Some(kind), Some(value)) if kind.eq_ignore_ascii_case("DOMAIN") => {
                let value = value.to_ascii_lowercase();
                self.domains.contains(&value) || self.suffix_ancestor(&value)
            }
            (Some(kind), Some(value)) if kind.eq_ignore_ascii_case("DOMAIN-SUFFIX") => {
                self.suffix_ancestor(&value.to_ascii_lowercase())
            }
            (Some(kind), Some(_))
                if kind.eq_ignore_ascii_case("IP-CIDR")
                    || kind.eq_ignore_ascii_case("IP-CIDR6") =>
            {
                parse_ip_rule(rule).map_or_else(
                    || self.other.contains(rule),
                    |network| {
                        self.ip_networks
                            .iter()
                            .any(|boundary| boundary.covers(network))
                    },
                )
            }
            _ => self.other.contains(rule),
        }
    }
}

fn read_adblock_boundary(
    root: &Path,
    apple: Option<&RuleBoundary>,
) -> Result<(RuleBoundary, usize), String> {
    let mut boundary = RuleBoundary::default();
    let mut apple_overlaps = 0;
    for entry in fs::read_dir(root.join("rulesets/AdBlock")).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("list") {
            let rules = read_rules(&path)?;
            if let Some(apple) = apple {
                apple_overlaps += overlap_count(&rules, apple);
            }
            for rule in rules {
                boundary.insert(&rule);
            }
        }
    }
    boundary.finish();
    Ok((boundary, apple_overlaps))
}

fn read_nsfw_protected_rules(root: &Path) -> Result<BTreeSet<String>, String> {
    let directory = root.join("rulesets/RULE-SET");
    let mut protected = BTreeSet::new();
    for name in NSFW_PROTECTED_CATEGORY_FILES {
        let path = directory.join(name);
        if path.is_file() {
            protected.extend(read_rules(&path)?);
        }
    }
    for domain in NSFW_PROTECTED_DOMAINS {
        protected.insert(format!("DOMAIN,{domain}"));
        protected.insert(format!("DOMAIN-SUFFIX,{domain}"));
    }
    Ok(protected)
}

fn overlap_count(rules: &BTreeSet<String>, boundary: &RuleBoundary) -> usize {
    rules
        .iter()
        .filter(|rule| boundary.intersects(rule))
        .count()
}

fn coverage_count(rules: &BTreeSet<String>, boundary: &RuleBoundary) -> usize {
    rules.iter().filter(|rule| boundary.covers(rule)).count()
}

fn remove_overlaps(rules: &mut BTreeSet<String>, boundary: &RuleBoundary) -> usize {
    let before = rules.len();
    rules.retain(|rule| !boundary.intersects(rule));
    before - rules.len()
}

fn remove_covered(rules: &mut BTreeSet<String>, boundary: &RuleBoundary) -> usize {
    let before = rules.len();
    rules.retain(|rule| !boundary.covers(rule));
    before - rules.len()
}

pub fn validate_category_boundaries(root: &Path) -> Result<Vec<String>, String> {
    let directory = root.join("rulesets/RULE-SET");
    let direct = read_rules(&directory.join("Direct.list"))?;
    let proxy = read_rules(&directory.join("GlobalProxy.list"))?;
    let mut apple_rules = BTreeSet::new();
    for name in ["Apple.list", "Apple_ip.list"] {
        let path = directory.join(name);
        if path.is_file() {
            apple_rules.extend(read_rules(&path)?);
        }
    }
    let apple = (!apple_rules.is_empty()).then(|| RuleBoundary::from_rules(&apple_rules));
    let cdn_path = directory.join("CDN.list");
    let cdn = if cdn_path.is_file() {
        Some(read_rules(&cdn_path)?)
    } else {
        None
    };
    let (adblock, apple_adblock) = read_adblock_boundary(root, apple.as_ref())?;
    let mut errors = Vec::new();
    if directory.join("Gaming_Web.list").exists() {
        let download = read_rules(&directory.join("Gaming_Download.list"))?;
        let play = read_rules(&directory.join("Gaming_Play.list"))?;
        let web = read_rules(&directory.join("Gaming_Web.list"))?;
        let download_boundary = RuleBoundary::from_rules(&download);
        if play.iter().any(|rule| download_boundary.intersects(rule)) {
            errors.push("Gaming Download and Play overlap".to_string());
        }
        if [&download, &play, &web].into_iter().any(|rules| {
            rules
                .iter()
                .any(|rule| !rule.starts_with("DOMAIN,") && !rule.starts_with("DOMAIN-SUFFIX,"))
        }) {
            errors
                .push("Gaming profiles contain ambiguous process/keyword/global rules".to_string());
        }
        if !web.is_disjoint(&download) || !web.is_disjoint(&play) {
            errors.push("Gaming Web contains exact Download/Play overlaps".to_string());
        }
    }
    let direct_adblock = coverage_count(&direct, &adblock);
    if direct_adblock > 0 {
        errors.push(format!(
            "Direct contains {direct_adblock} AdBlock-covered rule(s)"
        ));
    }
    let direct_proxy = direct.intersection(&proxy).count();
    if direct_proxy > 0 {
        errors.push(format!(
            "GlobalProxy contains {direct_proxy} exact Direct overlap(s)"
        ));
    }
    if let Some(apple) = &apple {
        let direct_apple = coverage_count(&direct, apple);
        if direct_apple > 0 {
            errors.push(format!(
                "Direct contains {direct_apple} Apple-owned rule(s); Apple routing belongs to Apple.list"
            ));
        }
        let proxy_apple = coverage_count(&proxy, apple);
        if proxy_apple > 0 {
            errors.push(format!(
                "GlobalProxy contains {proxy_apple} Apple-owned rule(s); Apple routing belongs to Apple.list"
            ));
        }
        if let Some(cdn) = &cdn {
            let cdn_apple = coverage_count(cdn, apple);
            if cdn_apple > 0 {
                errors.push(format!(
                    "CDN contains {cdn_apple} Apple-owned rule(s); Apple service routing belongs to Apple.list"
                ));
            }
        }
        for (name, message) in [
            (
                "GlobalProxy_ip",
                "GlobalProxy_ip contains {count} Apple-owned IP rule(s); Apple routing belongs to Apple_ip",
            ),
            (
                "CDN_ip",
                "CDN_ip contains {count} Apple-owned IP rule(s); Apple service routing belongs to Apple_ip",
            ),
        ] {
            let path = directory.join(format!("{name}.list"));
            if path.is_file() {
                let rules = read_rules(&path)?;
                let count = coverage_count(&rules, apple);
                if count > 0 {
                    errors.push(message.replace("{count}", &count.to_string()));
                }
            }
        }
    }
    let nsfw_path = directory.join("NSFW.list");
    if nsfw_path.is_file() {
        let nsfw = read_rules(&nsfw_path)?;
        let protected = read_nsfw_protected_rules(root)?;
        let protected = RuleBoundary::from_rules(&protected);
        let nsfw_shared = overlap_count(&nsfw, &protected);
        if nsfw_shared > 0 {
            errors.push(format!(
                "NSFW contains {nsfw_shared} shared-service overlap(s)"
            ));
        }
    }
    if apple_adblock > 0 {
        errors.push(format!(
            "AdBlock contains {apple_adblock} Apple service overlap(s)"
        ));
    }
    Ok(errors)
}

pub fn enforce_category_boundaries(root: &Path) -> Result<(usize, usize, usize, usize), String> {
    let directory = root.join("rulesets/RULE-SET");
    let direct_path = directory.join("Direct.list");
    let proxy_path = directory.join("GlobalProxy.list");
    let mut direct = read_rules(&direct_path)?;
    let mut proxy = read_rules(&proxy_path)?;

    let mut apple_rules = read_rules(&directory.join("Apple.list"))?;
    let apple_ip_path = directory.join("Apple_ip.list");
    if apple_ip_path.is_file() {
        apple_rules.extend(read_rules(&apple_ip_path)?);
    }
    let apple = RuleBoundary::from_rules(&apple_rules);
    let removed_apple_from_direct = remove_covered(&mut direct, &apple);
    let removed_apple_from_proxy = remove_covered(&mut proxy, &apple);
    let cdn_path = directory.join("CDN.list");
    let mut cdn = if cdn_path.is_file() {
        Some(read_rules(&cdn_path)?)
    } else {
        None
    };
    let removed_apple_from_cdn = cdn
        .as_mut()
        .map(|rules| remove_covered(rules, &apple))
        .unwrap_or(0);
    let global_proxy_ip_path = directory.join("GlobalProxy_ip.list");
    let mut global_proxy_ip = if global_proxy_ip_path.is_file() {
        Some(read_rules(&global_proxy_ip_path)?)
    } else {
        None
    };
    let removed_apple_from_global_proxy_ip = global_proxy_ip
        .as_mut()
        .map(|rules| remove_covered(rules, &apple))
        .unwrap_or(0);
    let cdn_ip_path = directory.join("CDN_ip.list");
    let mut cdn_ip = if cdn_ip_path.is_file() {
        Some(read_rules(&cdn_ip_path)?)
    } else {
        None
    };
    let removed_apple_from_cdn_ip = cdn_ip
        .as_mut()
        .map(|rules| remove_covered(rules, &apple))
        .unwrap_or(0);
    let (adblock, _) = read_adblock_boundary(root, None)?;

    let removed_from_direct = remove_covered(&mut direct, &adblock);
    let proxy_before = proxy.len();
    proxy.retain(|rule| !direct.contains(rule));
    let removed_direct_from_proxy = proxy_before - proxy.len();
    let removed_from_proxy = removed_apple_from_proxy + removed_direct_from_proxy;

    let nsfw_path = directory.join("NSFW.list");
    let removed_from_nsfw = if nsfw_path.is_file() {
        let mut nsfw = read_rules(&nsfw_path)?;
        let protected = read_nsfw_protected_rules(root)?;
        let protected = RuleBoundary::from_rules(&protected);
        let removed = remove_overlaps(&mut nsfw, &protected);
        write_rules(&nsfw_path, "NSFW", "REJECT", &nsfw)?;
        removed
    } else {
        0
    };

    write_rules(&direct_path, "Direct", "DIRECT", &direct)?;
    write_rules(&proxy_path, "GlobalProxy", "PROXY", &proxy)?;
    if removed_apple_from_cdn > 0
        && let Some(cdn) = &cdn
    {
        write_rules(&cdn_path, "CDN", "PROXY", cdn)?;
    }
    if removed_apple_from_global_proxy_ip > 0
        && let Some(rules) = &global_proxy_ip
    {
        write_rules(&global_proxy_ip_path, "GlobalProxy_ip", "PROXY", rules)?;
    }
    if removed_apple_from_cdn_ip > 0
        && let Some(rules) = &cdn_ip
    {
        write_rules(&cdn_ip_path, "CDN_ip", "PROXY", rules)?;
    }
    println!(
        "[INFO] category boundaries removed {removed_from_direct} AdBlock-covered and {removed_apple_from_direct} Apple-owned rule(s) from Direct, {removed_apple_from_proxy} Apple-owned and {removed_direct_from_proxy} Direct overlap(s) from GlobalProxy, {removed_apple_from_cdn} Apple-owned rule(s) from CDN, {removed_apple_from_global_proxy_ip} Apple-owned IP rule(s) from GlobalProxy_ip, {removed_apple_from_cdn_ip} Apple-owned IP rule(s) from CDN_ip, and {removed_from_nsfw} shared-service overlap(s) from NSFW"
    );
    Ok((
        removed_from_direct,
        removed_apple_from_direct,
        removed_from_proxy,
        removed_from_nsfw,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        discover_manifests, enforce_category_boundaries, manifest_header_policy, read_rules,
        remote_content, reverse_domain_labels, split_gaming_rules, validate_category_boundaries,
    };
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    #[test]
    fn apple_header_recommends_direct_without_inline_policy() {
        assert_eq!(manifest_header_policy("Apple"), "DIRECT");
        assert_eq!(manifest_header_policy("AppleNews"), "PROXY");
        let header = crate::generate_header(
            "Apple",
            manifest_header_policy("Apple"),
            "Any",
            "Apple service domains; routing is selected by the consumer",
            1,
        );
        assert!(header.lines().any(|line| line == "# Policy: DIRECT"));
        assert!(header.lines().all(|line| line.starts_with('#')));
    }

    fn rule_matches_host(rule: &str, host: &str) -> bool {
        let Some((kind, value)) = rule.split_once(',') else {
            return false;
        };
        let host = host.to_ascii_lowercase();
        let value = value.to_ascii_lowercase();
        match kind.to_ascii_uppercase().as_str() {
            "DOMAIN" => host == value,
            "DOMAIN-SUFFIX" => host == value || host.ends_with(&format!(".{value}")),
            "DOMAIN-KEYWORD" => host.contains(&value),
            "DOMAIN-WILDCARD" => {
                let pattern = regex::escape(&value).replace(r"\*", ".*");
                regex::Regex::new(&format!("^{pattern}$"))
                    .is_ok_and(|pattern| pattern.is_match(&host))
            }
            "DOMAIN-REGEX" => {
                regex::Regex::new(&value).is_ok_and(|pattern| pattern.is_match(&host))
            }
            _ => false,
        }
    }

    #[test]
    fn apple_music_hosts_are_owned_by_apple_and_not_generic_routing_categories() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root.join("rulesets/RULE-SET");
        let apple = read_rules(&directory.join("Apple.list")).unwrap();
        let apple_boundary = super::RuleBoundary::from_rules(&apple);
        let music_hosts = [
            "music.apple.com",
            "amp-api-edge.music.apple.com",
            "audio-ssl.itunes.apple.com",
            "aod-ssl.itunes.apple.com",
            "is1-ssl.mzstatic.com",
            "s.mzstatic.com",
        ];

        for host in music_hosts {
            assert!(
                apple_boundary.covers(&format!("DOMAIN,{host}")),
                "Apple.list should own {host}"
            );
            for category in ["GlobalProxy", "CDN", "AppleNews"] {
                let rules = read_rules(&directory.join(format!("{category}.list"))).unwrap();
                let matching: Vec<_> = rules
                    .iter()
                    .filter(|rule| rule_matches_host(rule, host))
                    .collect();
                assert!(
                    matching.is_empty(),
                    "{host} also matches {category}: {matching:?}"
                );
            }
        }
    }

    #[test]
    fn remote_content_joins_borrowed_sources_in_order_including_empty_bodies() {
        let urls = ["first", "empty", "last", "missing"]
            .map(str::to_string)
            .to_vec();
        let downloads = HashMap::from([
            ("first".to_string(), "alpha".to_string()),
            ("empty".to_string(), String::new()),
            ("last".to_string(), "omega".to_string()),
        ]);

        assert_eq!(remote_content(&urls, &downloads), "alpha\n\nomega");
    }

    #[test]
    fn reverse_domain_labels_avoids_intermediate_storage_without_changing_labels() {
        assert_eq!(reverse_domain_labels("www.example.com"), "com.example.www");
        assert_eq!(reverse_domain_labels("bücher.example"), "example.bücher");
        assert_eq!(reverse_domain_labels("example."), ".example");
    }

    #[test]
    fn gaming_split_prioritizes_endpoints_and_keeps_shared_domains_in_web() {
        let root = std::env::temp_dir().join(format!("script-hub-gaming-{}", std::process::id()));
        let rules = root.join("rulesets/RULE-SET");
        let sources = root.join("rulesets/Sources/custom");
        fs::create_dir_all(&rules).unwrap();
        fs::create_dir_all(&sources).unwrap();
        let legacy = "DOMAIN-SUFFIX,epicgames.com\nDOMAIN,download.epicgames.com\nDOMAIN,match.epicgames.com\nDOMAIN-SUFFIX,patchy-game.test\nDOMAIN,login.live.com\nDOMAIN-KEYWORD,epicgames\nPROCESS-NAME,launcher.exe\n";
        fs::write(rules.join("Gaming.list"), legacy).unwrap();
        fs::write(
            rules.join("Gaming_Download.list"),
            "DOMAIN-SUFFIX,game-cdn.example\n",
        )
        .unwrap();
        fs::write(
            sources.join("Gaming_Download_manual.list"),
            "# download\nDOMAIN,download.epicgames.com\n",
        )
        .unwrap();
        fs::write(
            sources.join("Gaming_Play_manual.list"),
            "DOMAIN,match.epicgames.com\n",
        )
        .unwrap();
        split_gaming_rules(&root).unwrap();
        let download = fs::read_to_string(rules.join("Gaming_Download.list")).unwrap();
        assert!(download.contains("DOMAIN-SUFFIX,game-cdn.example"));
        assert!(download.contains("DOMAIN,download.epicgames.com"));
        let web = fs::read_to_string(rules.join("Gaming_Web.list")).unwrap();
        assert!(web.contains("DOMAIN-SUFFIX,epicgames.com"));
        assert!(web.contains("DOMAIN-SUFFIX,patchy-game.test"));
        assert!(!web.contains("download.epicgames.com"));
        assert!(!web.contains("match.epicgames.com"));
        assert!(!web.contains("login.live.com"));
        assert!(!web.contains("PROCESS-NAME") && !web.contains("DOMAIN-KEYWORD"));
        assert_eq!(
            fs::read_to_string(rules.join("Gaming.list")).unwrap(),
            legacy
        );
        fs::write(
            sources.join("Gaming_Play_manual.list"),
            "DOMAIN-SUFFIX,epicgames.com\n",
        )
        .unwrap();
        assert!(split_gaming_rules(&root).is_err());
        assert_eq!(
            fs::read_to_string(rules.join("Gaming_Web.list")).unwrap(),
            web
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn category_boundaries_remove_only_mutually_exclusive_exact_rules() {
        let root =
            std::env::temp_dir().join(format!("script-hub-general-update-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("rulesets/RULE-SET")).unwrap();
        fs::create_dir_all(root.join("rulesets/AdBlock")).unwrap();
        fs::write(
            root.join("rulesets/RULE-SET/Direct.list"),
            "DOMAIN,cn.example\nDOMAIN,track.ads.example\nDOMAIN-SUFFIX,shop.test\n",
        )
        .unwrap();
        fs::write(
            root.join("rulesets/RULE-SET/GlobalProxy.list"),
            "DOMAIN,cn.example\nDOMAIN,global.example\n",
        )
        .unwrap();
        fs::write(
            root.join("rulesets/RULE-SET/SocialMedia.list"),
            "DOMAIN-SUFFIX,social.example\n",
        )
        .unwrap();
        fs::write(
            root.join("rulesets/RULE-SET/NSFW.list"),
            "DOMAIN-SUFFIX,adult.example\nDOMAIN,cdn.social.example\nDOMAIN-SUFFIX,crunchyroll.com\nDOMAIN-SUFFIX,10bet.com\n",
        )
        .unwrap();
        fs::write(
            root.join("rulesets/RULE-SET/Apple.list"),
            "DOMAIN-SUFFIX,apple.example\n",
        )
        .unwrap();
        fs::write(
            root.join("rulesets/AdBlock/Advertising.list"),
            "DOMAIN-SUFFIX,ads.example\nDOMAIN,ad.shop.test\n",
        )
        .unwrap();

        assert_eq!(validate_category_boundaries(&root).unwrap().len(), 3);
        assert_eq!(enforce_category_boundaries(&root).unwrap(), (1, 0, 1, 3));
        assert!(validate_category_boundaries(&root).unwrap().is_empty());
        let direct = fs::read_to_string(root.join("rulesets/RULE-SET/Direct.list")).unwrap();
        let proxy = fs::read_to_string(root.join("rulesets/RULE-SET/GlobalProxy.list")).unwrap();
        let nsfw = fs::read_to_string(root.join("rulesets/RULE-SET/NSFW.list")).unwrap();
        assert!(direct.contains("# Policy: DIRECT"));
        assert!(direct.contains("DOMAIN,cn.example"));
        assert!(direct.contains("DOMAIN-SUFFIX,shop.test"));
        assert!(!direct.contains("track.ads.example"));
        assert!(proxy.contains("DOMAIN,global.example"));
        assert!(!proxy.contains("cn.example"));
        assert!(nsfw.contains("DOMAIN-SUFFIX,adult.example"));
        assert!(!nsfw.contains("social.example"));
        assert!(!nsfw.contains("crunchyroll.com"));
        assert!(!nsfw.contains("10bet.com"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn category_boundaries_detect_apple_adblock_hierarchy() {
        let root =
            std::env::temp_dir().join(format!("script-hub-apple-boundary-{}", std::process::id()));
        let rules = root.join("rulesets/RULE-SET");
        let adblock = root.join("rulesets/AdBlock");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&rules).unwrap();
        fs::create_dir_all(&adblock).unwrap();
        fs::write(
            rules.join("Direct.list"),
            "DOMAIN,direct.example\nDOMAIN,weather.apple.test\n",
        )
        .unwrap();
        fs::write(
            rules.join("GlobalProxy.list"),
            "DOMAIN,proxy.example\nDOMAIN,store.apple.test\nDOMAIN-SUFFIX,shared.test\n",
        )
        .unwrap();
        fs::write(
            rules.join("Apple.list"),
            "DOMAIN-SUFFIX,apple.test\nDOMAIN,asset.shared.test\n",
        )
        .unwrap();
        fs::write(
            adblock.join("Advertising.list"),
            "DOMAIN,metrics.apple.test\n",
        )
        .unwrap();
        fs::write(
            adblock.join("Telemetry.list"),
            "DOMAIN,notifications.apple.test\n",
        )
        .unwrap();

        let errors = validate_category_boundaries(&root).unwrap();
        assert_eq!(
            errors,
            vec![
                "Direct contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
                "GlobalProxy contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
                "AdBlock contains 2 Apple service overlap(s)",
            ]
        );
        assert_eq!(enforce_category_boundaries(&root).unwrap(), (0, 1, 1, 0));
        let direct = fs::read_to_string(rules.join("Direct.list")).unwrap();
        let proxy = fs::read_to_string(rules.join("GlobalProxy.list")).unwrap();
        assert!(!direct.contains("weather.apple.test"));
        assert!(!proxy.contains("store.apple.test"));
        assert!(proxy.contains("DOMAIN-SUFFIX,shared.test"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn category_boundaries_remove_only_apple_owned_cdn_rules() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-apple-cdn-boundary-{}",
            std::process::id()
        ));
        let rules = root.join("rulesets/RULE-SET");
        let adblock = root.join("rulesets/AdBlock");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&rules).unwrap();
        fs::create_dir_all(&adblock).unwrap();
        fs::write(rules.join("Direct.list"), "DOMAIN,direct.example\n").unwrap();
        fs::write(rules.join("GlobalProxy.list"), "DOMAIN,proxy.example\n").unwrap();
        fs::write(
            rules.join("Apple.list"),
            "DOMAIN-SUFFIX,apple.com\nDOMAIN-SUFFIX,itunes.apple.com\nDOMAIN-SUFFIX,mzstatic.com\n",
        )
        .unwrap();
        let cdn = rules.join("CDN.list");
        fs::write(
            &cdn,
            "DOMAIN,hls-svod.itunes.apple.com\nDOMAIN,music.apple.com\nDOMAIN-SUFFIX,akamaized.net\nDOMAIN,cdn.fastly.net\n",
        )
        .unwrap();

        assert_eq!(
            validate_category_boundaries(&root).unwrap(),
            vec!["CDN contains 2 Apple-owned rule(s); Apple service routing belongs to Apple.list"]
        );
        assert_eq!(enforce_category_boundaries(&root).unwrap(), (0, 0, 0, 0));
        assert!(validate_category_boundaries(&root).unwrap().is_empty());
        let cdn = fs::read_to_string(cdn).unwrap();
        assert!(!cdn.contains("hls-svod.itunes.apple.com"));
        assert!(!cdn.contains("music.apple.com"));
        assert!(cdn.contains("DOMAIN-SUFFIX,akamaized.net"));
        assert!(cdn.contains("DOMAIN,cdn.fastly.net"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn category_boundaries_move_only_apple_covered_networks_to_apple_ip() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-apple-ip-boundary-{}",
            std::process::id()
        ));
        let rules = root.join("rulesets/RULE-SET");
        let adblock = root.join("rulesets/AdBlock");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&rules).unwrap();
        fs::create_dir_all(&adblock).unwrap();
        fs::write(rules.join("Direct.list"), "DOMAIN,direct.example\n").unwrap();
        fs::write(rules.join("GlobalProxy.list"), "DOMAIN,proxy.example\n").unwrap();
        fs::write(rules.join("Apple.list"), "DOMAIN-SUFFIX,apple.com\n").unwrap();
        fs::write(
            rules.join("Apple_ip.list"),
            "IP-CIDR,17.0.0.0/8,NO-RESOLVE\n",
        )
        .unwrap();
        fs::write(
            rules.join("GlobalProxy_ip.list"),
            "IP-CIDR,17.252.156.0/23,NO-RESOLVE\nIP-CIDR,17.0.0.0/7,NO-RESOLVE\nIP-CIDR6,2001:db8::/32,NO-RESOLVE\n",
        )
        .unwrap();
        fs::write(
            rules.join("CDN_ip.list"),
            "IP-CIDR,17.252.156.0/23,NO-RESOLVE\nIP-CIDR6,2606:4700::/32,NO-RESOLVE\n",
        )
        .unwrap();
        let apple_news = "DOMAIN,gspe1-ssl.ls.apple.com\nDOMAIN,news-client-search.apple.com\n";
        fs::write(rules.join("AppleNews.list"), apple_news).unwrap();

        assert_eq!(
            validate_category_boundaries(&root).unwrap(),
            vec![
                "GlobalProxy_ip contains 1 Apple-owned IP rule(s); Apple routing belongs to Apple_ip",
                "CDN_ip contains 1 Apple-owned IP rule(s); Apple service routing belongs to Apple_ip",
            ]
        );
        assert_eq!(enforce_category_boundaries(&root).unwrap(), (0, 0, 0, 0));
        assert!(validate_category_boundaries(&root).unwrap().is_empty());

        let proxy_ip = fs::read_to_string(rules.join("GlobalProxy_ip.list")).unwrap();
        assert!(!proxy_ip.contains("17.252.156.0/23"));
        assert!(proxy_ip.contains("17.0.0.0/7"));
        assert!(proxy_ip.contains("2001:db8::/32"));
        let cdn_ip = fs::read_to_string(rules.join("CDN_ip.list")).unwrap();
        assert!(!cdn_ip.contains("17.252.156.0/23"));
        assert!(cdn_ip.contains("2606:4700::/32"));
        assert_eq!(
            fs::read_to_string(rules.join("AppleNews.list")).unwrap(),
            apple_news
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manifest_comments_are_not_treated_as_local_paths() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-general-manifest-{}",
            std::process::id()
        ));
        let links = root.join("rulesets/Sources/Links");
        fs::create_dir_all(&links).unwrap();
        fs::write(
            links.join("Direct_sources.list"),
            "# description\n../local.list\nhttps://example.test/direct.list\n",
        )
        .unwrap();

        let manifests = discover_manifests(&root).unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].local_paths.len(), 1);
        assert_eq!(manifests[0].remote_urls.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
