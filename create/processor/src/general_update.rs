//! Rust-owned refresh for the non-PROMAX ruleset manifests.

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
    let remote_content = manifest
        .remote_urls
        .iter()
        .filter_map(|url| downloads.get(url))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let policy = match manifest.name.as_str() {
        "Direct" => "DIRECT",
        "NSFW" => "REJECT",
        _ => "PROXY",
    };
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
    let mut content = crate::generate_header(
        name,
        policy,
        "Any",
        "Rust-merged canonical ruleset",
        rules.len(),
    );
    for rule in rules {
        content.push_str(rule);
        content.push('\n');
    }
    crate::safe_write_file_internal(path, &content, true)
        .then_some(())
        .ok_or_else(|| format!("failed to publish {}", path.display()))
}

#[derive(Default)]
struct RuleBoundary {
    domains: HashSet<String>,
    suffixes: HashSet<String>,
    other: HashSet<String>,
    reversed_hosts: Vec<String>,
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
            _ => {
                self.other.insert(rule.to_string());
            }
        }
    }

    fn finish(&mut self) {
        self.reversed_hosts = self
            .domains
            .iter()
            .chain(&self.suffixes)
            .map(|host| host.split('.').rev().collect::<Vec<_>>().join("."))
            .collect();
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
        let prefix = format!("{}.", suffix.split('.').rev().collect::<Vec<_>>().join("."));
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
            _ => self.other.contains(rule),
        }
    }
}

fn read_adblock_boundary(root: &Path) -> Result<RuleBoundary, String> {
    let mut boundary = RuleBoundary::default();
    for entry in fs::read_dir(root.join("rulesets/AdBlock"))
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("list") {
            for rule in read_rules(&path)? {
                boundary.insert(&rule);
            }
        }
    }
    boundary.finish();
    Ok(boundary)
}

fn count_adblock_overlaps(root: &Path, boundary: &RuleBoundary) -> Result<usize, String> {
    let mut count = 0;
    for entry in fs::read_dir(root.join("rulesets/AdBlock"))
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("list") {
            count += read_rules(&path)?
                .iter()
                .filter(|rule| boundary.intersects(rule))
                .count();
        }
    }
    Ok(count)
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
    let adblock = read_adblock_boundary(root)?;
    let apple_path = directory.join("Apple.list");
    let apple = if apple_path.is_file() {
        let rules = read_rules(&apple_path)?;
        Some(RuleBoundary::from_rules(&rules))
    } else {
        None
    };
    let mut errors = Vec::new();
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
    if let Some(apple) = &apple {
        let apple_adblock = count_adblock_overlaps(root, apple)?;
        if apple_adblock > 0 {
            errors.push(format!(
                "AdBlock contains {apple_adblock} Apple service overlap(s)"
            ));
        }
    }
    Ok(errors)
}

pub fn enforce_category_boundaries(root: &Path) -> Result<(usize, usize, usize, usize), String> {
    let directory = root.join("rulesets/RULE-SET");
    let direct_path = directory.join("Direct.list");
    let proxy_path = directory.join("GlobalProxy.list");
    let mut direct = read_rules(&direct_path)?;
    let mut proxy = read_rules(&proxy_path)?;

    let apple_rules = read_rules(&directory.join("Apple.list"))?;
    let apple = RuleBoundary::from_rules(&apple_rules);
    let removed_apple_from_direct = remove_covered(&mut direct, &apple);
    let removed_apple_from_proxy = remove_covered(&mut proxy, &apple);
    let adblock = read_adblock_boundary(root)?;

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
    println!(
        "[INFO] category boundaries removed {removed_from_direct} AdBlock-covered and {removed_apple_from_direct} Apple-owned rule(s) from Direct, {removed_apple_from_proxy} Apple-owned and {removed_direct_from_proxy} Direct overlap(s) from GlobalProxy, and {removed_from_nsfw} shared-service overlap(s) from NSFW"
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
    use super::{discover_manifests, enforce_category_boundaries, validate_category_boundaries};
    use std::fs;

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

        let errors = validate_category_boundaries(&root).unwrap();
        assert_eq!(
            errors,
            vec![
                "Direct contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
                "GlobalProxy contains 1 Apple-owned rule(s); Apple routing belongs to Apple.list",
                "AdBlock contains 1 Apple service overlap(s)",
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
