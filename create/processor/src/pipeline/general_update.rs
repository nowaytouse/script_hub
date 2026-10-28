//! Rust-owned refresh for the non-PROMAX ruleset manifests.

use crate::promax::safety::IpNetwork;
use crate::promax::source::{DownloadConfig, Downloader};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
    "us.list",
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

const INTERNAL_RULESETS: &[&str] = &[
    "NSFW",
    "StreamJP",
    "StreamHK",
    "StreamTW",
    "AppleNews",
    "CDN",
    "Cloudflare",
    "substore",
    "Gaming",
    "Pixiv",
    "Spotify",
    "StreamEU",
    "StreamUS",
    "Binance",
];

fn input_path(root: &Path, name: &str, suffix: &str) -> PathBuf {
    let public = root.join(format!("rulesets/RULE-SET/{name}{suffix}.list"));
    if public.is_file() || !INTERNAL_RULESETS.contains(&name) {
        public
    } else {
        root.join(format!("rulesets/Sources/compiled/{name}{suffix}.list"))
    }
}

/// Internal categories are build inputs, not backwards-compatible subscriptions.
pub fn prepare_ruleset_inputs(root: &Path) -> Result<(), String> {
    let snapshot = root.join("rulesets/Sources/compiled");
    fs::create_dir_all(&snapshot).map_err(|e| e.to_string())?;
    for name in INTERNAL_RULESETS {
        let source = snapshot.join(format!("{name}.list"));
        if !source.is_file() {
            let paths: Vec<PathBuf> = ["", "_ip", "_extra"]
                .iter()
                .map(|suffix| root.join(format!("rulesets/RULE-SET/{name}{suffix}.list")))
                .filter(|path| path.is_file())
                .collect();
            if paths.is_empty() {
                return Err(format!(
                    "missing internal ruleset input {name}; run a full upstream update"
                ));
            }
            crate::process_ruleset(crate::RulesetInput {
                name,
                target_dir: &snapshot,
                conflict_domains: &[],
                skip_conflict: false,
                description: "Internal upstream snapshot; not a subscription",
                local_paths: &paths,
                remote_content: "",
            })?;
        }
        let snapshot_paths: Vec<PathBuf> = ["", "_ip", "_extra"]
            .iter()
            .map(|suffix| snapshot.join(format!("{name}{suffix}.list")))
            .filter(|path| path.is_file())
            .collect();
        crate::process_ruleset(crate::RulesetInput {
            name,
            target_dir: &snapshot,
            conflict_domains: &[],
            skip_conflict: false,
            description: "Internal typed upstream snapshot; not a subscription",
            local_paths: &snapshot_paths,
            remote_content: "",
        })?;
        for suffix in ["", "_ip", "_extra"] {
            let from = snapshot.join(format!("{name}{suffix}.list"));
            match fs::read_to_string(&from) {
                Ok(content) => {
                    let to = root.join(format!("rulesets/RULE-SET/{name}{suffix}.list"));
                    if !crate::safe_write_file_internal(&to, &content, true) {
                        return Err(format!("cannot prepare {}", to.display()));
                    }
                }
                Err(e) if !suffix.is_empty() && e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("cannot prepare {}: {e}", from.display())),
            }
        }
    }
    Ok(())
}

/// Remove only retired public names, after their cleaned inputs were persisted.
pub fn retire_ruleset_inputs(root: &Path) -> Result<usize, String> {
    let snapshot = root.join("rulesets/Sources/compiled");
    fs::create_dir_all(&snapshot).map_err(|e| e.to_string())?;
    for name in INTERNAL_RULESETS {
        for suffix in ["", "_ip", "_extra"] {
            let from = root.join(format!("rulesets/RULE-SET/{name}{suffix}.list"));
            match fs::read_to_string(&from) {
                Ok(content) => {
                    let to = snapshot.join(format!("{name}{suffix}.list"));
                    if !crate::safe_write_file_internal(&to, &content, true) {
                        return Err(format!("cannot preserve {}", to.display()));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("cannot preserve {}: {e}", from.display())),
            }
        }
    }
    let mut removed = 0;
    for name in INTERNAL_RULESETS
        .iter()
        .copied()
        .chain(std::iter::once("AdBlock"))
    {
        for suffix in ["", "_ip", "_extra"] {
            for directory in ["rulesets/RULE-SET", "rulesets/RULE-SET/github"] {
                let path = root.join(format!("{directory}/{name}{suffix}.list"));
                match fs::remove_file(&path) {
                    Ok(()) => removed += 1,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("cannot retire {}: {e}", path.display())),
                }
            }
            let path = root.join(format!("rulesets/SingBox/{name}{suffix}_Singbox.srs"));
            match fs::remove_file(&path) {
                Ok(()) => removed += 1,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("cannot retire {}: {e}", path.display())),
            }
        }
    }
    let adblock = root.join("rulesets/AdBlock");
    if adblock.is_dir() {
        for entry in fs::read_dir(&adblock).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                continue;
            }
            let file = entry.file_name().to_string_lossy().into_owned();
            if !file.starts_with("AdBlock_") || !file.ends_with(".list") {
                continue;
            }
            let content = fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
            if content.starts_with("# Retired PROMAX shard") && parse_rules(&content).is_empty() {
                for path in [entry.path(), adblock.join("github").join(&file)] {
                    match fs::remove_file(&path) {
                        Ok(()) => removed += 1,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                        Err(error) => {
                            return Err(format!("cannot retire {}: {error}", path.display()));
                        }
                    }
                }
            }
        }
        for file in ["reject-drop.list", "reject-no-drop.list"] {
            for directory in ["rulesets/AdBlock", "rulesets/AdBlock/github"] {
                let path = root.join(directory).join(file);
                match fs::remove_file(&path) {
                    Ok(()) => removed += 1,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("cannot retire {}: {e}", path.display())),
                }
            }
        }
    }
    Ok(removed)
}

/// Normalize every upstream-derived category, including Direct, through the
/// same splitter. Non-domain matchers survive only in explicit extra files.
pub fn split_published_rulesets(root: &Path) -> Result<(), String> {
    let directory = root.join("rulesets/RULE-SET");
    let mut names = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().map_err(|e| e.to_string())?.is_file()
            && let Some(stem) = name.strip_suffix(".list")
            && stem != "AdBlock"
        {
            names.push(
                stem.strip_suffix("_ip")
                    .or_else(|| stem.strip_suffix("_extra"))
                    .unwrap_or(stem)
                    .to_string(),
            );
        }
    }
    names.sort();
    names.dedup();
    for name in names {
        let paths: Vec<PathBuf> = ["", "_ip", "_extra"]
            .iter()
            .map(|suffix| directory.join(format!("{name}{suffix}.list")))
            .filter(|path| path.is_file())
            .collect();
        let mut count = 0;
        for path in &paths {
            count += read_rules(path)?.len();
        }
        if count == 0 {
            for path in paths {
                fs::remove_file(&path).map_err(|error| {
                    format!("cannot remove empty ruleset {}: {error}", path.display())
                })?;
            }
        } else {
            crate::process_ruleset(crate::RulesetInput {
                name: &name,
                target_dir: &directory,
                conflict_domains: &[],
                skip_conflict: false,
                description: "Current typed ruleset; policy selected by consumer",
                local_paths: &paths,
                remote_content: "",
            })?;
        }
    }
    let adblock = root.join("rulesets/AdBlock");
    let httpdns: Vec<PathBuf> = ["HTTPDNS_Hijack.list", "HTTPDNS_Hijack_ip.list"]
        .iter()
        .map(|name| adblock.join(name))
        .filter(|path| path.is_file())
        .collect();
    if !httpdns.is_empty() {
        crate::process_ruleset(crate::RulesetInput {
            name: "HTTPDNS_Hijack",
            target_dir: &adblock,
            conflict_domains: &[],
            skip_conflict: false,
            description: "HTTPDNS endpoints; domain and IP matchers published separately",
            local_paths: &httpdns,
            remote_content: "",
        })?;
    }
    Ok(())
}

pub fn validate_published_layout(root: &Path) -> Result<Vec<String>, String> {
    let directory = root.join("rulesets/RULE-SET");
    let mut errors = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(name) = file.strip_suffix(".list") else {
            continue;
        };
        let base = name
            .strip_suffix("_ip")
            .or_else(|| name.strip_suffix("_extra"))
            .unwrap_or(name);
        if INTERNAL_RULESETS.contains(&base) || base == "AdBlock" {
            errors.push(format!("retired subscription remains published: {file}"));
        }
        let rules = read_rules(&entry.path())?;
        if rules.is_empty() {
            errors.push(format!("empty ruleset remains published: {file}"));
        }
        for rule in rules {
            let kind = rule.split(',').next().unwrap_or("");
            let domain = matches!(
                kind,
                "DOMAIN" | "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" | "DOMAIN-WILDCARD" | "DOMAIN-REGEX"
            );
            let ip = matches!(kind, "IP-CIDR" | "IP-CIDR6" | "IP-ASN" | "GEOIP");
            if (name.ends_with("_ip") && !ip)
                || (name.ends_with("_extra") && (domain || ip))
                || (!name.ends_with("_ip") && !name.ends_with("_extra") && !domain)
            {
                errors.push(format!(
                    "{file} contains a matcher outside its declared type: {kind}"
                ));
                break;
            }
        }
    }
    Ok(errors)
}

pub fn enforce_jp_ownership(root: &Path) -> Result<usize, String> {
    let mut owned = read_rules(&root.join("rulesets/Sources/custom/JP_manual.list"))?;
    owned.extend(read_rules(&input_path(root, "Pixiv", ""))?);
    let boundary = RuleBoundary::from_rules(&owned);
    let mut jp = read_rules(&root.join("rulesets/RULE-SET/JP.list"))?;
    jp.extend(owned);
    write_rules(&root.join("rulesets/RULE-SET/JP.list"), "JP", &jp)?;
    let directory = root.join("rulesets/RULE-SET");
    let mut removed = 0;
    for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(name) = file.strip_suffix(".list") else {
            continue;
        };
        if name == "JP" || name == "Pixiv" || name.ends_with("_ip") || name.ends_with("_extra") {
            continue;
        }
        let mut rules = read_rules(&entry.path())?;
        let count = remove_covered(&mut rules, &boundary);
        if count > 0 {
            write_rules(&entry.path(), name, &rules)?;
            removed += count;
        }
    }
    Ok(removed)
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
            missing_remote
                .iter()
                .map(|url| crate::redact_url(url))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let target = root.join(if INTERNAL_RULESETS.contains(&manifest.name.as_str()) {
        "rulesets/Sources/compiled"
    } else {
        "rulesets/RULE-SET"
    });
    fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    let mut remote_content = remote_content(&manifest.remote_urls, downloads);
    if manifest.name == "Apple" {
        let aliases = read_apple_alias_rules(root)?;
        if !aliases.is_empty() {
            remote_content.push('\n');
            for alias in aliases {
                remote_content.push_str(&alias);
                remote_content.push('\n');
            }
        }
    }
    // AdsGram is an advertising platform, not a messaging/collaboration service.
    // Do not turn a service classification into an ad-platform routing overlay.
    let conflicts = if manifest.name == "SocialMedia" {
        vec!["adsgram.ai".to_string()]
    } else if manifest.name == "Pixiv" {
        // The ecosystem aggregate also names its advertising domain.
        vec!["ads-pixiv.net".to_string()]
    } else {
        Vec::new()
    };
    crate::process_ruleset(crate::RulesetInput {
        name: &manifest.name,
        target_dir: &target,
        conflict_domains: &conflicts,
        skip_conflict: false,
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
            crate::redact_url(&failure.url),
            failure.error
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
    Ok(parse_rules(&content))
}

fn parse_rules(content: &str) -> BTreeSet<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            if line.starts_with("IP-") || line.starts_with("GEOIP,") {
                crate::normalize_rule(line).unwrap_or_else(|| line.to_string())
            } else {
                line.to_string()
            }
        })
        .collect()
}

fn read_optional_rules(path: &Path) -> Result<BTreeSet<String>, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(parse_rules(&content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::verify_missing_directory(path)?;
            Ok(BTreeSet::new())
        }
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

const CONSOLIDATED_RULESETS: &[(&str, &[&str])] = &[
    ("us", &["StreamUS", "StreamEU", "Spotify"]),
    ("JP", &["NSFW", "StreamJP", "Pixiv"]),
    ("TW", &["StreamTW", "StreamHK"]),
    ("Apple", &["Apple", "AppleNews"]),
    (
        "GlobalProxy",
        &["GlobalProxy", "CDN", "Cloudflare", "substore", "Binance"],
    ),
];

/// Rebuild the requested subscription unions from sanitized categories. Legacy
/// inputs keep their scope; a quick update reuses their last validated snapshot.
pub fn consolidate_rulesets(root: &Path) -> Result<(), String> {
    let directory = root.join("rulesets/RULE-SET");
    let mut outputs = Vec::new();
    for (target, inputs) in CONSOLIDATED_RULESETS {
        let mut domains = BTreeSet::new();
        for input in *inputs {
            domains.extend(read_rules(&input_path(root, input, ""))?);
            domains.extend(read_optional_rules(&input_path(root, input, "_ip"))?);
            domains.extend(read_optional_rules(&input_path(root, input, "_extra"))?);
        }
        if *target == "JP" {
            domains.extend(read_rules(
                &root.join("rulesets/Sources/custom/JP_manual.list"),
            )?);
        }
        if domains.is_empty() {
            return Err(format!("refusing empty consolidated ruleset {target}"));
        }
        outputs.push((target.to_string(), domains));
    }
    // Read every dependency before changing any target: a missing leaf must not
    // publish an incomplete union or replace the previous good subscription.
    for (name, rules) in outputs {
        crate::process_ruleset(crate::RulesetInput {
            name: &name,
            target_dir: &directory,
            conflict_domains: &[],
            skip_conflict: false,
            description: "Current consolidated subscription; policy selected by consumer",
            local_paths: &[],
            remote_content: &rules.into_iter().collect::<Vec<_>>().join("\n"),
        })?;
    }
    Ok(())
}

fn validate_consolidated_rulesets(root: &Path) -> Result<Vec<String>, String> {
    let directory = root.join("rulesets/RULE-SET");
    if !root
        .join("rulesets/Sources/custom/JP_manual.list")
        .is_file()
        && !directory.join("JP.list").is_file()
        && !directory.join("TW.list").is_file()
    {
        return Ok(Vec::new());
    }
    let mut errors = Vec::new();
    for (target, inputs) in CONSOLIDATED_RULESETS {
        let output = read_rules(&directory.join(format!("{target}.list")))?;
        let output_ip = read_optional_rules(&directory.join(format!("{target}_ip.list")))?;
        let output_extra = read_optional_rules(&directory.join(format!("{target}_extra.list")))?;
        for input in *inputs {
            if !read_rules(&input_path(root, input, ""))?.is_subset(&output)
                || !read_optional_rules(&input_path(root, input, "_ip"))?.is_subset(&output_ip)
                || !read_optional_rules(&input_path(root, input, "_extra"))?
                    .is_subset(&output_extra)
            {
                errors.push(format!("{target} is missing consolidated {input} rules"));
            }
        }
        if *target == "JP"
            && !read_rules(&root.join("rulesets/Sources/custom/JP_manual.list"))?.is_subset(&output)
        {
            errors.push("JP is missing explicitly maintained services".to_string());
        }
    }
    Ok(errors)
}

fn write_rules(path: &Path, name: &str, rules: &BTreeSet<String>) -> Result<(), String> {
    if rules.is_empty() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "cannot remove empty ruleset {}: {error}",
                path.display()
            )),
        };
    }
    let description = match name {
        "JP" => {
            "Japan-oriented subscription: NSFW, Japanese streaming and explicit services; not a location assertion"
        }
        "TW" => "Taiwan and Hong Kong streaming services",
        "Gaming_Download" => "Game packages and patches",
        "Gaming_Play" => "Game networking and matchmaking",
        "Gaming_Web" => {
            "Stores, accounts, communities and shared-platform fallback; not a gameplay-only list"
        }
        _ => "Rust-merged canonical ruleset",
    };
    let mut content = crate::generate_header(name, description, rules.len());
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
    let legacy = read_rules(&input_path(root, "Gaming", ""))?;
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
        write_rules(&directory.join(format!("{name}.list")), name, rules)?;
        write_rules(&mirror.join(format!("{name}.list")), name, rules)?;
    }
    let extra = read_optional_rules(&input_path(root, "Gaming", "_extra"))?;
    if !extra.is_empty() {
        write_rules(
            &directory.join("Gaming_Web_extra.list"),
            "Gaming_Web_extra",
            &extra,
        )?;
    }
    let ips = read_optional_rules(&input_path(root, "Gaming", "_ip"))?;
    write_rules(&directory.join("Gaming_Web_ip.list"), "Gaming_Web_ip", &ips)?;
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
                apple_overlaps += rules
                    .iter()
                    .filter(|rule| {
                        let reviewed_ad_host = crate::promax::rule::SurgeRule::parse(rule)
                            .is_ok_and(|parsed| {
                                crate::promax::safety::is_reviewed_exact_privacy_domain(&parsed)
                                    && !crate::promax::safety::apple_host_pattern_is_critical(
                                        &parsed.payload,
                                    )
                            });
                        apple.intersects(rule) && !reviewed_ad_host
                    })
                    .count();
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
    let mut protected = BTreeSet::new();
    for name in NSFW_PROTECTED_CATEGORY_FILES {
        let stem = name.trim_end_matches(".list");
        let path = input_path(root, stem, "");
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

const APPLE_OWNERSHIP_EXCLUDED_RULESETS: &[&str] = &[
    "Apple.list",
    "Apple_ip.list",
    "AppleNews.list",
    "AppleNews_ip.list",
    // Legacy ad rules are compiled from rulesets/AdBlock under a separate guard.
    "AdBlock.list",
];

fn apple_ownership_category_paths(directory: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("list") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if APPLE_OWNERSHIP_EXCLUDED_RULESETS.contains(&file_name) {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        paths.push((name.to_string(), path));
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(paths)
}

fn read_apple_manual_domain_rules(root: &Path) -> Result<BTreeSet<String>, String> {
    let manual = root.join("rulesets/Sources/custom/Apple_manual.list");
    let mut rules = BTreeSet::new();
    if manual.is_file() {
        rules.extend(
            read_rules(&manual)?
                .into_iter()
                .filter(|rule| rule.starts_with("DOMAIN,") || rule.starts_with("DOMAIN-SUFFIX,")),
        );
    }
    Ok(rules)
}

fn read_apple_alias_rules(root: &Path) -> Result<BTreeSet<String>, String> {
    // Follow the maintained Apple DNS mapping only for exact CNAME-style
    // aliases under shared CDN/DNS providers; never import provider suffixes.
    let aliases = root.join("rulesets/Sources/dns/mapping/DNS_Global_Apple.list");
    let mut rules = BTreeSet::new();
    if aliases.is_file() {
        for rule in read_rules(&aliases)? {
            let Some(host) = rule.strip_prefix("DOMAIN,") else {
                continue;
            };
            let host = host.to_ascii_lowercase();
            if [".akadns.net", ".edgesuite.net", ".edgekey.net"]
                .iter()
                .any(|suffix| host.ends_with(suffix))
            {
                rules.insert(rule);
            }
        }
    }
    Ok(rules)
}

fn read_apple_source_domain_rules(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut rules = read_apple_manual_domain_rules(root)?;
    rules.extend(read_apple_alias_rules(root)?);
    Ok(rules)
}

fn read_apple_ownership_rules(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut rules = BTreeSet::new();
    for name in [
        "Apple.list",
        "Apple_ip.list",
        "AppleNews.list",
        "AppleNews_ip.list",
    ] {
        let stem = name.trim_end_matches(".list");
        let (base, suffix) = stem
            .strip_suffix("_ip")
            .map_or((stem, ""), |base| (base, "_ip"));
        let path = input_path(root, base, suffix);
        if path.is_file() {
            rules.extend(read_rules(&path)?);
        }
    }
    rules.extend(read_apple_source_domain_rules(root)?);
    Ok(rules)
}

fn sync_apple_owner_domains(root: &Path) -> Result<usize, String> {
    let path = root.join("rulesets/RULE-SET/Apple.list");
    let mut owner = if path.is_file() {
        read_rules(&path)?
    } else {
        BTreeSet::new()
    };
    let before = owner.len();
    owner.extend(read_apple_source_domain_rules(root)?);
    let added = owner.len() - before;
    if added > 0 {
        write_rules(&path, "Apple", &owner)?;
    }
    Ok(added)
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
    let apple_rules = read_apple_ownership_rules(root)?;
    let apple = (!apple_rules.is_empty()).then(|| RuleBoundary::from_rules(&apple_rules));
    let cdn_path = input_path(root, "CDN", "");
    let cdn = if cdn_path.is_file() {
        Some(read_rules(&cdn_path)?)
    } else {
        None
    };
    let (adblock, apple_adblock) = read_adblock_boundary(root, apple.as_ref())?;
    let mut errors = validate_consolidated_rulesets(root)?;
    let apple_owner_domains = read_apple_source_domain_rules(root)?;
    if !apple_owner_domains.is_empty() {
        let owner_path = directory.join("Apple.list");
        let owner_rules = if owner_path.is_file() {
            read_rules(&owner_path)?
        } else {
            BTreeSet::new()
        };
        let owner_boundary = RuleBoundary::from_rules(&owner_rules);
        let missing = apple_owner_domains
            .iter()
            .filter(|rule| !owner_boundary.covers(rule))
            .count();
        if missing > 0 {
            errors.push(format!(
                "Apple.list is missing {missing} curated Apple service domain(s)"
            ));
        }
    }
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
    let mut requested_proxy_additions = BTreeSet::new();
    for name in ["CDN", "Cloudflare", "substore", "Binance"] {
        requested_proxy_additions.extend(read_optional_rules(&input_path(root, name, ""))?);
    }
    let direct_proxy = direct
        .intersection(&proxy)
        .filter(|rule| !requested_proxy_additions.contains(*rule))
        .count();
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
            let (base, suffix) = name
                .strip_suffix("_ip")
                .map_or((name, ""), |base| (base, "_ip"));
            let path = input_path(root, base, suffix);
            if path.is_file() {
                let rules = read_rules(&path)?;
                let count = coverage_count(&rules, apple);
                if count > 0 {
                    errors.push(message.replace("{count}", &count.to_string()));
                }
            }
        }
        for (name, path) in apple_ownership_category_paths(&directory)? {
            if matches!(
                name.as_str(),
                "Direct" | "GlobalProxy" | "CDN" | "GlobalProxy_ip" | "CDN_ip"
            ) {
                continue;
            }
            let rules = read_rules(&path)?;
            let count = coverage_count(&rules, apple);
            if count > 0 {
                let owner = if name.ends_with("_ip") {
                    "Apple_ip"
                } else {
                    "Apple.list"
                };
                errors.push(format!(
                    "{name} contains {count} Apple-owned rule(s); Apple routing belongs to {owner}"
                ));
            }
        }
    }
    let nsfw_path = input_path(root, "NSFW", "");
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
    let synced_apple_owner_domains = sync_apple_owner_domains(root)?;
    let direct_path = directory.join("Direct.list");
    let proxy_path = directory.join("GlobalProxy.list");
    let mut direct = read_rules(&direct_path)?;
    let mut proxy = read_rules(&proxy_path)?;

    let apple_rules = read_apple_ownership_rules(root)?;
    let apple = RuleBoundary::from_rules(&apple_rules);
    let removed_apple_from_direct = remove_covered(&mut direct, &apple);
    let removed_apple_from_proxy = remove_covered(&mut proxy, &apple);
    let cdn_path = input_path(root, "CDN", "");
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
    let cdn_ip_path = input_path(root, "CDN", "_ip");
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

    let nsfw_path = input_path(root, "NSFW", "");
    let removed_from_nsfw = if nsfw_path.is_file() {
        let mut nsfw = read_rules(&nsfw_path)?;
        let protected = read_nsfw_protected_rules(root)?;
        let protected = RuleBoundary::from_rules(&protected);
        let removed = remove_overlaps(&mut nsfw, &protected);
        write_rules(&nsfw_path, "NSFW", &nsfw)?;
        removed
    } else {
        0
    };

    write_rules(&direct_path, "Direct", &direct)?;
    write_rules(&proxy_path, "GlobalProxy", &proxy)?;
    if removed_apple_from_cdn > 0
        && let Some(cdn) = &cdn
    {
        write_rules(&cdn_path, "CDN", cdn)?;
    }
    if removed_apple_from_global_proxy_ip > 0
        && let Some(rules) = &global_proxy_ip
    {
        write_rules(&global_proxy_ip_path, "GlobalProxy_ip", rules)?;
    }
    if removed_apple_from_cdn_ip > 0
        && let Some(rules) = &cdn_ip
    {
        write_rules(&cdn_ip_path, "CDN_ip", rules)?;
    }
    let mut removed_apple_by_category = BTreeMap::new();
    for (name, path) in apple_ownership_category_paths(&directory)? {
        if matches!(
            name.as_str(),
            "Direct" | "GlobalProxy" | "CDN" | "GlobalProxy_ip" | "CDN_ip"
        ) {
            continue;
        }
        let mut rules = read_rules(&path)?;
        let removed = remove_covered(&mut rules, &apple);
        if removed > 0 {
            write_rules(&path, &name, &rules)?;
            removed_apple_by_category.insert(name, removed);
        }
    }
    let category_counts = removed_apple_by_category
        .iter()
        .map(|(name, count)| format!("{name}={count}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "[INFO] category boundaries synced {synced_apple_owner_domains} curated Apple service domain(s) into Apple.list; removed {removed_from_direct} AdBlock-covered and {removed_apple_from_direct} Apple-owned rule(s) from Direct, {removed_apple_from_proxy} Apple-owned and {removed_direct_from_proxy} Direct overlap(s) from GlobalProxy, {removed_apple_from_cdn} Apple-owned rule(s) from CDN, {removed_apple_from_global_proxy_ip} Apple-owned IP rule(s) from GlobalProxy_ip, {removed_apple_from_cdn_ip} Apple-owned IP rule(s) from CDN_ip, and {removed_from_nsfw} shared-service overlap(s) from NSFW; other Apple ownership removals: [{}]",
        category_counts,
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
        Manifest, discover_manifests, enforce_category_boundaries, process_manifest, read_rules,
        remote_content, reverse_domain_labels, split_gaming_rules, validate_category_boundaries,
    };
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    fn consolidation_fixture() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "script-hub-consolidation-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let rules = root.join("rulesets/RULE-SET");
        fs::create_dir_all(&rules).unwrap();
        let custom = root.join("rulesets/Sources/custom");
        fs::create_dir_all(&custom).unwrap();
        fs::write(custom.join("JP_manual.list"), "DOMAIN-SUFFIX,misskey.io\n").unwrap();
        for name in [
            "NSFW",
            "Pixiv",
            "StreamJP",
            "StreamTW",
            "StreamHK",
            "Apple",
            "AppleNews",
            "GlobalProxy",
            "CDN",
            "Cloudflare",
            "substore",
            "Spotify",
            "StreamEU",
            "StreamUS",
            "Binance",
            "Gaming",
        ] {
            fs::write(
                rules.join(format!("{name}.list")),
                format!("DOMAIN-SUFFIX,{}.example.test\n", name.to_ascii_lowercase()),
            )
            .unwrap();
        }
        root
    }

    #[test]
    fn consolidation_preserves_leaf_scope_and_merges_only_requested_services() {
        let root = consolidation_fixture();
        let rules = root.join("rulesets/RULE-SET");
        let nsfw = fs::read(rules.join("NSFW.list")).unwrap();
        let jp_stream = fs::read(rules.join("StreamJP.list")).unwrap();
        fs::write(
            rules.join("CDN_ip.list"),
            "IP-CIDR,198.51.100.0/24,no-resolve\n",
        )
        .unwrap();
        fs::write(rules.join("CDN_extra.list"), "USER-AGENT,Example*\n").unwrap();
        super::consolidate_rulesets(&root).unwrap();
        let jp = read_rules(&rules.join("JP.list")).unwrap();
        assert_eq!(jp.len(), 4);
        assert!(jp.contains("DOMAIN-SUFFIX,misskey.io"));
        assert_eq!(read_rules(&rules.join("TW.list")).unwrap().len(), 2);
        assert_eq!(read_rules(&rules.join("Apple.list")).unwrap().len(), 2);
        assert_eq!(
            read_rules(&rules.join("GlobalProxy.list")).unwrap().len(),
            5
        );
        assert_eq!(
            read_rules(&rules.join("GlobalProxy_ip.list")).unwrap(),
            read_rules(&rules.join("CDN_ip.list")).unwrap()
        );
        assert_eq!(fs::read(rules.join("NSFW.list")).unwrap(), nsfw);
        assert_eq!(fs::read(rules.join("StreamJP.list")).unwrap(), jp_stream);
        let before = fs::read(rules.join("JP.list")).unwrap();
        super::consolidate_rulesets(&root).unwrap();
        assert_eq!(fs::read(rules.join("JP.list")).unwrap(), before);
        assert!(
            super::validate_consolidated_rulesets(&root)
                .unwrap()
                .is_empty()
        );
        fs::remove_file(rules.join("GlobalProxy_extra.list")).unwrap();
        assert!(
            super::validate_consolidated_rulesets(&root)
                .unwrap()
                .iter()
                .any(|error| error.contains("GlobalProxy is missing consolidated CDN"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn us_and_binance_unions_preserve_types_refreshes_and_retire_old_public_names() {
        let root = consolidation_fixture();
        let directory = root.join("rulesets/RULE-SET");
        fs::write(
            directory.join("Spotify_ip.list"),
            "IP-CIDR,192.0.2.0/24,no-resolve\n",
        )
        .unwrap();
        fs::write(
            directory.join("Spotify_extra.list"),
            "PROCESS-NAME,Spotify\n",
        )
        .unwrap();
        fs::write(
            directory.join("StreamEU_extra.list"),
            "USER-AGENT,Europe*\n",
        )
        .unwrap();
        fs::write(
            directory.join("StreamUS_extra.list"),
            "USER-AGENT,America*\n",
        )
        .unwrap();
        fs::write(
            directory.join("Binance_ip.list"),
            "IP-CIDR,198.51.100.0/24,no-resolve\n",
        )
        .unwrap();
        super::prepare_ruleset_inputs(&root).unwrap();
        super::consolidate_rulesets(&root).unwrap();
        super::retire_ruleset_inputs(&root).unwrap();
        assert_eq!(read_rules(&directory.join("us.list")).unwrap().len(), 3);
        assert_eq!(read_rules(&directory.join("us_ip.list")).unwrap().len(), 1);
        assert_eq!(
            read_rules(&directory.join("us_extra.list")).unwrap().len(),
            3
        );
        assert!(
            read_rules(&directory.join("GlobalProxy.list"))
                .unwrap()
                .contains("DOMAIN-SUFFIX,binance.example.test")
        );
        assert!(
            read_rules(&directory.join("GlobalProxy_ip.list"))
                .unwrap()
                .contains("IP-CIDR,198.51.100.0/24,no-resolve")
        );
        for old in ["Spotify", "StreamEU", "StreamUS", "Binance"] {
            assert!(!directory.join(format!("{old}.list")).exists());
            assert!(
                root.join(format!("rulesets/Sources/compiled/{old}.list"))
                    .is_file()
            );
        }
        let source = root.join("rulesets/Sources/compiled/Spotify.list");
        fs::write(source, "DOMAIN-SUFFIX,new-spotify.example.test\n").unwrap();
        super::prepare_ruleset_inputs(&root).unwrap();
        super::consolidate_rulesets(&root).unwrap();
        super::retire_ruleset_inputs(&root).unwrap();
        let current = read_rules(&directory.join("us.list")).unwrap();
        assert!(current.contains("DOMAIN-SUFFIX,new-spotify.example.test"));
        assert!(!current.contains("DOMAIN-SUFFIX,spotify.example.test"));
        assert!(
            super::validate_consolidated_rulesets(&root)
                .unwrap()
                .is_empty()
        );
        assert!(super::validate_published_layout(&root).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn consolidation_rebuilds_regions_and_follows_refreshed_base_removals() {
        let root = consolidation_fixture();
        let rules = root.join("rulesets/RULE-SET");
        super::consolidate_rulesets(&root).unwrap();
        for name in ["NSFW", "CDN", "AppleNews"] {
            fs::write(
                rules.join(format!("{name}.list")),
                format!(
                    "DOMAIN-SUFFIX,new-{}.example.test\n",
                    name.to_ascii_lowercase()
                ),
            )
            .unwrap();
        }
        // A full manifest update refreshes these bases before consolidation.
        fs::write(
            rules.join("Apple.list"),
            "DOMAIN-SUFFIX,apple.example.test\n",
        )
        .unwrap();
        fs::write(
            rules.join("GlobalProxy.list"),
            "DOMAIN-SUFFIX,globalproxy.example.test\n",
        )
        .unwrap();
        super::consolidate_rulesets(&root).unwrap();
        for (target, removed) in [
            ("JP", "nsfw"),
            ("Apple", "applenews"),
            ("GlobalProxy", "cdn"),
        ] {
            let content = read_rules(&rules.join(format!("{target}.list"))).unwrap();
            assert!(!content.contains(&format!("DOMAIN-SUFFIX,{removed}.example.test")));
            assert!(content.contains(&format!("DOMAIN-SUFFIX,new-{removed}.example.test")));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_consolidation_dependency_preserves_all_existing_outputs() {
        let root = consolidation_fixture();
        let rules = root.join("rulesets/RULE-SET");
        super::consolidate_rulesets(&root).unwrap();
        let before = fs::read(rules.join("JP.list")).unwrap();
        let apple = fs::read(rules.join("Apple.list")).unwrap();
        fs::write(rules.join("NSFW.list"), "DOMAIN-SUFFIX,new.example.test\n").unwrap();
        fs::remove_file(rules.join("CDN.list")).unwrap();
        assert!(super::consolidate_rulesets(&root).is_err());
        assert_eq!(fs::read(rules.join("JP.list")).unwrap(), before);
        assert_eq!(fs::read(rules.join("Apple.list")).unwrap(), apple);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_does_not_let_regional_unions_consume_legacy_members() {
        let root = consolidation_fixture();
        let rules = root.join("rulesets/RULE-SET");
        fs::create_dir_all(root.join("rulesets/AdBlock")).unwrap();
        fs::write(
            rules.join("Direct.list"),
            "DOMAIN-SUFFIX,direct.example.test\n",
        )
        .unwrap();
        super::consolidate_rulesets(&root).unwrap();
        let nsfw = read_rules(&rules.join("NSFW.list")).unwrap();
        let hk = read_rules(&rules.join("StreamHK.list")).unwrap();
        for _ in 0..2 {
            crate::smart_cleanup::run_cleanup(&root.to_string_lossy()).unwrap();
            enforce_category_boundaries(&root).unwrap();
            super::consolidate_rulesets(&root).unwrap();
            assert_eq!(read_rules(&rules.join("NSFW.list")).unwrap(), nsfw);
            assert_eq!(read_rules(&rules.join("StreamHK.list")).unwrap(), hk);
            assert!(
                super::validate_consolidated_rulesets(&root)
                    .unwrap()
                    .is_empty()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn consolidated_coverage_guard_rejects_missing_members() {
        let root = consolidation_fixture();
        super::consolidate_rulesets(&root).unwrap();
        fs::write(
            root.join("rulesets/RULE-SET/JP.list"),
            "DOMAIN-SUFFIX,misskey.io\n",
        )
        .unwrap();
        assert!(
            !super::validate_consolidated_rulesets(&root)
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn latest_publication_retires_aliases_but_keeps_reusable_internal_inputs() {
        let root = consolidation_fixture();
        let directory = root.join("rulesets/RULE-SET");
        let adblock = root.join("rulesets/AdBlock");
        fs::create_dir_all(adblock.join("github")).unwrap();
        for (name, header) in [
            (
                "AdBlock_Old_01.list",
                "# Retired PROMAX shard: upstream coverage changed.\n",
            ),
            (
                "AdBlock_Old_02.list",
                "# Retired PROMAX shard; refresh required.\n",
            ),
        ] {
            fs::write(adblock.join(name), header).unwrap();
            fs::write(adblock.join("github").join(name), header).unwrap();
        }
        fs::write(
            adblock.join("AdBlock_User.list"),
            "# User-owned empty list\n",
        )
        .unwrap();
        fs::write(
            directory.join("Gaming.list"),
            "DOMAIN-SUFFIX,game.example.test\n",
        )
        .unwrap();
        super::prepare_ruleset_inputs(&root).unwrap();
        super::consolidate_rulesets(&root).unwrap();
        super::split_published_rulesets(&root).unwrap();
        super::retire_ruleset_inputs(&root).unwrap();
        assert!(!adblock.join("AdBlock_Old_01.list").exists());
        assert!(!adblock.join("github/AdBlock_Old_02.list").exists());
        assert!(adblock.join("AdBlock_User.list").exists());
        assert!(!directory.join("NSFW.list").exists());
        assert!(!directory.join("Gaming.list").exists());
        assert!(root.join("rulesets/Sources/compiled/NSFW.list").is_file());
        assert!(!directory.join("JP_ip.list").exists());
        assert!(super::validate_published_layout(&root).unwrap().is_empty());
        let before = fs::read(directory.join("JP.list")).unwrap();
        super::prepare_ruleset_inputs(&root).unwrap();
        super::consolidate_rulesets(&root).unwrap();
        super::retire_ruleset_inputs(&root).unwrap();
        assert_eq!(before, fs::read(directory.join("JP.list")).unwrap());
        assert!(
            super::validate_consolidated_rulesets(&root)
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn jp_ownership_removes_pixiv_children_without_claiming_shared_cdn_parents() {
        let root = consolidation_fixture();
        let directory = root.join("rulesets/RULE-SET");
        fs::write(
            root.join("rulesets/Sources/custom/JP_manual.list"),
            "DOMAIN-SUFFIX,misskey.io\nDOMAIN-SUFFIX,pixiv.net\nDOMAIN-SUFFIX,pximg.net\n",
        )
        .unwrap();
        fs::write(
            directory.join("GlobalProxy.list"),
            "DOMAIN-SUFFIX,pixiv.net\nDOMAIN,i.pximg.net\nDOMAIN-SUFFIX,cloudfront.net\n",
        )
        .unwrap();
        super::consolidate_rulesets(&root).unwrap();
        super::enforce_jp_ownership(&root).unwrap();
        let global = read_rules(&directory.join("GlobalProxy.list")).unwrap();
        assert!(!global.contains("DOMAIN-SUFFIX,pixiv.net"));
        assert!(!global.contains("DOMAIN,i.pximg.net"));
        assert!(global.contains("DOMAIN-SUFFIX,cloudfront.net"));
        assert!(
            read_rules(&directory.join("JP.list"))
                .unwrap()
                .contains("DOMAIN-SUFFIX,pixiv.net")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn category_headers_leave_routing_to_the_consumer() {
        let header = crate::generate_header(
            "Apple",
            "Apple service domains; routing is selected by the consumer",
            1,
        );
        assert!(header.lines().any(|line| line == "# Policy: USER-DEFINED"));
        assert!(header.lines().all(|line| line.starts_with('#')));
    }

    #[test]
    fn social_service_sources_exclude_an_ad_platform_without_blocking_services() {
        let root =
            std::env::temp_dir().join(format!("script-hub-collaboration-{}", std::process::id()));
        let manifest = super::Manifest {
            name: "SocialMedia".to_string(),
            local_paths: Vec::new(),
            remote_urls: vec!["https://fixture.test/communication.list".to_string()],
        };
        let downloads = HashMap::from([(
            manifest.remote_urls[0].clone(),
            "+.slack.com\n+.adsgram.ai\n+.matrix.org\n".to_string(),
        )]);
        super::process_manifest(&root, &manifest, &downloads).unwrap();
        let path = root.join("rulesets/RULE-SET/SocialMedia.list");
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("adsgram.ai"));
        assert!(content.contains("DOMAIN-SUFFIX,slack.com\n"));
        assert!(content.contains("DOMAIN-SUFFIX,matrix.org\n"));
        assert!(content.contains("# Policy: USER-DEFINED\n"));
        let before = fs::read(&path).unwrap();
        super::process_manifest(&root, &manifest, &downloads).unwrap();
        assert_eq!(fs::read(path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
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
                let rules = read_rules(&super::input_path(&root, category, "")).unwrap();
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
        assert!(direct.contains("# Policy: USER-DEFINED"));
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
    fn apple_ad_boundary_allows_only_reviewed_exact_ad_hosts() {
        let root = consolidation_fixture();
        let adblock = root.join("rulesets/AdBlock");
        fs::create_dir_all(&adblock).unwrap();
        let apple = super::RuleBoundary::from_rules(&std::collections::BTreeSet::from([
            "DOMAIN-SUFFIX,apple.com".into(),
            "DOMAIN-SUFFIX,icloud.com".into(),
        ]));
        let path = adblock.join("fixture.list");
        fs::write(
            &path,
            "DOMAIN,api-adservices.apple.com\nDOMAIN,iadsdk.apple.com\n",
        )
        .unwrap();
        let (boundary, overlaps) = super::read_adblock_boundary(&root, Some(&apple)).unwrap();
        assert_eq!(overlaps, 0);
        assert!(boundary.intersects("DOMAIN,api-adservices.apple.com"));
        fs::write(&path, "DOMAIN-SUFFIX,api-adservices.apple.com\nDOMAIN,metrics.icloud.com\nDOMAIN,account.apple.com\n").unwrap();
        assert_eq!(
            super::read_adblock_boundary(&root, Some(&apple)).unwrap().1,
            3
        );
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
    fn apple_ownership_is_enforced_across_categories_without_broad_parent_removal() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-apple-all-category-boundary-{}",
            std::process::id()
        ));
        let rules = root.join("rulesets/RULE-SET");
        let adblock = root.join("rulesets/AdBlock");
        let custom = root.join("rulesets/Sources/custom");
        let apple_mapping = root.join("rulesets/Sources/dns/mapping");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&rules).unwrap();
        fs::create_dir_all(&adblock).unwrap();
        fs::create_dir_all(&custom).unwrap();
        fs::create_dir_all(&apple_mapping).unwrap();

        fs::write(rules.join("Direct.list"), "DOMAIN,direct.example\n").unwrap();
        fs::write(rules.join("GlobalProxy.list"), "DOMAIN,proxy.example\n").unwrap();
        fs::write(
            rules.join("Apple.list"),
            "DOMAIN-SUFFIX,apple.com\nDOMAIN,store.apple.com.edgekey.net\nDOMAIN,ocsp-lb.apple.com.akadns.net\nDOMAIN,icloud-cdn.icloud.com.akadns.net\n",
        )
        .unwrap();
        fs::write(
            custom.join("Apple_manual.list"),
            "DOMAIN,manual-alias.apple.test\n",
        )
        .unwrap();
        fs::write(
            rules.join("Google.list"),
            "DOMAIN,store.apple.com.edgekey.net\nDOMAIN-SUFFIX,akadns.net\n",
        )
        .unwrap();
        fs::write(
            rules.join("Microsoft.list"),
            "DOMAIN,ocsp-lb.apple.com.akadns.net\nDOMAIN-SUFFIX,edgesuite.net\n",
        )
        .unwrap();
        fs::write(
            rules.join("StreamUS.list"),
            "DOMAIN-SUFFIX,itunes.apple.com\nDOMAIN-SUFFIX,amazonaws.com\n",
        )
        .unwrap();
        fs::write(
            apple_mapping.join("DNS_Global_Apple.list"),
            "DOMAIN,login.apple.com.edgesuite.net\nDOMAIN-SUFFIX,edgesuite.net\nDOMAIN-SUFFIX,akadns.net\n",
        )
        .unwrap();
        fs::write(
            rules.join("SocialMedia.list"),
            "DOMAIN,login.apple.com.edgesuite.net\nDOMAIN-SUFFIX,edgesuite.net\nDOMAIN-SUFFIX,akadns.net\n",
        )
        .unwrap();
        fs::write(
            rules.join("Gaming.list"),
            "DOMAIN,icloud-cdn.icloud.com.akadns.net\nDOMAIN-SUFFIX,game-cdn.example\n",
        )
        .unwrap();
        let apple_news = "DOMAIN,gspe1-ssl.ls.apple.com\nDOMAIN,manual-alias.apple.test\n";
        fs::write(rules.join("AppleNews.list"), apple_news).unwrap();
        fs::write(
            rules.join("AdBlock.list"),
            "DOMAIN,manual-alias.apple.test\n",
        )
        .unwrap();

        assert_eq!(
            validate_category_boundaries(&root).unwrap(),
            vec![
                "Apple.list is missing 2 curated Apple service domain(s)",
                "Gaming contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
                "Google contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
                "Microsoft contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
                "SocialMedia contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
                "StreamUS contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
            ]
        );
        assert_eq!(enforce_category_boundaries(&root).unwrap(), (0, 0, 0, 0));
        assert!(validate_category_boundaries(&root).unwrap().is_empty());
        let apple = fs::read_to_string(rules.join("Apple.list")).unwrap();
        assert!(apple.contains("DOMAIN,manual-alias.apple.test"));
        assert!(apple.contains("DOMAIN,login.apple.com.edgesuite.net"));

        for (name, alias, preserved_parent) in [
            (
                "Gaming",
                "icloud-cdn.icloud.com.akadns.net",
                "game-cdn.example",
            ),
            ("Google", "store.apple.com.edgekey.net", "akadns.net"),
            ("Microsoft", "ocsp-lb.apple.com.akadns.net", "edgesuite.net"),
            (
                "SocialMedia",
                "login.apple.com.edgesuite.net",
                "edgesuite.net",
            ),
            ("StreamUS", "itunes.apple.com", "amazonaws.com"),
        ] {
            let content = fs::read_to_string(rules.join(format!("{name}.list"))).unwrap();
            assert!(!content.contains(alias), "{name} retained {alias}");
            assert!(
                content.contains(preserved_parent),
                "{name} lost {preserved_parent}"
            );
            let entries: Vec<_> = content
                .lines()
                .filter(|line| !line.starts_with('#'))
                .collect();
            let sorted: std::collections::BTreeSet<_> = entries.iter().copied().collect();
            assert_eq!(entries, sorted.into_iter().collect::<Vec<_>>());
        }
        assert_eq!(
            fs::read_to_string(rules.join("AppleNews.list")).unwrap(),
            apple_news
        );
        assert!(
            fs::read_to_string(rules.join("AdBlock.list"))
                .unwrap()
                .contains("manual-alias.apple.test")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn apple_refresh_publishes_exact_aliases_from_maintained_mapping() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-apple-alias-refresh-{}",
            std::process::id()
        ));
        let mapping = root.join("rulesets/Sources/dns/mapping");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&mapping).unwrap();
        fs::write(
            mapping.join("DNS_Global_Apple.list"),
            "DOMAIN,ocsp-lb.apple.com.akadns.net\nDOMAIN-SUFFIX,akadns.net\n",
        )
        .unwrap();

        process_manifest(
            &root,
            &Manifest {
                name: "Apple".to_string(),
                local_paths: Vec::new(),
                remote_urls: Vec::new(),
            },
            &HashMap::new(),
        )
        .unwrap();

        let apple = fs::read_to_string(root.join("rulesets/RULE-SET/Apple.list")).unwrap();
        assert!(apple.contains("DOMAIN,ocsp-lb.apple.com.akadns.net"));
        assert!(!apple.contains("DOMAIN-SUFFIX,akadns.net"));
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
