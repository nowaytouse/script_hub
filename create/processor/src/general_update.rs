//! Rust-owned refresh for the non-PROMAX ruleset manifests.

use crate::promax::source::{DownloadConfig, Downloader};
use std::collections::{BTreeSet, HashMap};
use std::ffi::CString;
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

const NSFW_SHARED_INFRASTRUCTURE: &[&str] = &[
    "cdn.discordapp.com",
    "cdn.statically.io",
    "images.pexels.com",
    "images.weserv.nl",
    "media.discordapp.net",
    "pinterest.com",
    "retrocrush.tv",
    "tubi.tv",
    "tumblr.com",
    "uploads-ssl.webflow.com",
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
        attempts: 2,
        timeout: Duration::from_secs(45),
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
    let local_paths = manifest
        .local_paths
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");
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
    let values = [
        manifest.name.as_str(),
        &target.to_string_lossy(),
        "",
        policy,
        "Any",
        "Rust-merged canonical ruleset",
        &local_paths,
        &remote_content,
    ];
    let strings: Vec<CString> = values
        .iter()
        .map(|value| CString::new(*value).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    let ok = crate::process_ruleset_ffi(
        strings[0].as_ptr(),
        strings[1].as_ptr(),
        strings[2].as_ptr(),
        false,
        strings[3].as_ptr(),
        strings[4].as_ptr(),
        strings[5].as_ptr(),
        strings[6].as_ptr(),
        strings[7].as_ptr(),
    );
    ok.then_some(())
        .ok_or_else(|| format!("Rust ruleset merge failed: {}", manifest.name))
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

fn read_adblock_rules(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut adblock = BTreeSet::new();
    for entry in fs::read_dir(root.join("rulesets/AdBlock"))
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("list") {
            adblock.extend(read_rules(&path)?);
        }
    }
    Ok(adblock)
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
    for domain in NSFW_SHARED_INFRASTRUCTURE {
        protected.insert(format!("DOMAIN,{domain}"));
        protected.insert(format!("DOMAIN-SUFFIX,{domain}"));
    }
    Ok(protected)
}

pub fn validate_category_boundaries(root: &Path) -> Result<Vec<String>, String> {
    let directory = root.join("rulesets/RULE-SET");
    let direct = read_rules(&directory.join("Direct.list"))?;
    let proxy = read_rules(&directory.join("GlobalProxy.list"))?;
    let adblock = read_adblock_rules(root)?;
    let mut errors = Vec::new();
    let direct_adblock = direct.intersection(&adblock).count();
    if direct_adblock > 0 {
        errors.push(format!(
            "Direct contains {direct_adblock} exact AdBlock overlap(s)"
        ));
    }
    let direct_proxy = direct.intersection(&proxy).count();
    if direct_proxy > 0 {
        errors.push(format!(
            "GlobalProxy contains {direct_proxy} exact Direct overlap(s)"
        ));
    }
    let nsfw_path = directory.join("NSFW.list");
    if nsfw_path.is_file() {
        let nsfw = read_rules(&nsfw_path)?;
        let protected = read_nsfw_protected_rules(root)?;
        let nsfw_shared = nsfw.intersection(&protected).count();
        if nsfw_shared > 0 {
            errors.push(format!(
                "NSFW contains {nsfw_shared} shared-service overlap(s)"
            ));
        }
    }
    Ok(errors)
}

pub fn enforce_category_boundaries(root: &Path) -> Result<(usize, usize, usize), String> {
    let directory = root.join("rulesets/RULE-SET");
    let direct_path = directory.join("Direct.list");
    let proxy_path = directory.join("GlobalProxy.list");
    let mut direct = read_rules(&direct_path)?;
    let mut proxy = read_rules(&proxy_path)?;

    let adblock = read_adblock_rules(root)?;

    let direct_before = direct.len();
    direct.retain(|rule| !adblock.contains(rule));
    let removed_from_direct = direct_before - direct.len();
    let proxy_before = proxy.len();
    proxy.retain(|rule| !direct.contains(rule));
    let removed_from_proxy = proxy_before - proxy.len();

    let nsfw_path = directory.join("NSFW.list");
    let removed_from_nsfw = if nsfw_path.is_file() {
        let mut nsfw = read_rules(&nsfw_path)?;
        let protected = read_nsfw_protected_rules(root)?;
        let before = nsfw.len();
        nsfw.retain(|rule| !protected.contains(rule));
        write_rules(&nsfw_path, "NSFW", "REJECT", &nsfw)?;
        before - nsfw.len()
    } else {
        0
    };

    write_rules(&direct_path, "Direct", "DIRECT", &direct)?;
    write_rules(&proxy_path, "GlobalProxy", "PROXY", &proxy)?;
    println!(
        "[INFO] category boundaries removed {removed_from_direct} AdBlock overlap(s) from Direct, {removed_from_proxy} Direct overlap(s) from GlobalProxy, and {removed_from_nsfw} shared-service overlap(s) from NSFW"
    );
    Ok((removed_from_direct, removed_from_proxy, removed_from_nsfw))
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
            "DOMAIN,cn.example\nDOMAIN,ads.example\n",
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
            "DOMAIN-SUFFIX,adult.example\nDOMAIN-SUFFIX,social.example\n",
        )
        .unwrap();
        fs::write(
            root.join("rulesets/AdBlock/Advertising.list"),
            "DOMAIN,ads.example\n",
        )
        .unwrap();

        assert_eq!(validate_category_boundaries(&root).unwrap().len(), 3);
        assert_eq!(enforce_category_boundaries(&root).unwrap(), (1, 1, 1));
        assert!(validate_category_boundaries(&root).unwrap().is_empty());
        let direct = fs::read_to_string(root.join("rulesets/RULE-SET/Direct.list")).unwrap();
        let proxy = fs::read_to_string(root.join("rulesets/RULE-SET/GlobalProxy.list")).unwrap();
        let nsfw = fs::read_to_string(root.join("rulesets/RULE-SET/NSFW.list")).unwrap();
        assert!(direct.contains("# Policy: DIRECT"));
        assert!(direct.contains("DOMAIN,cn.example"));
        assert!(!direct.contains("ads.example"));
        assert!(proxy.contains("DOMAIN,global.example"));
        assert!(!proxy.contains("cn.example"));
        assert!(nsfw.contains("DOMAIN-SUFFIX,adult.example"));
        assert!(!nsfw.contains("social.example"));
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
