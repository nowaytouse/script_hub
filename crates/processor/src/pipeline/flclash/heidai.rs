//! Shared local rule projection of the complete 217heidai AdGuard release.
use super::{
    MAIN_MODULE, MAX_PROMAX_BYTES, OWNER, PROMAX_PROTECTED_SERVICE_HOSTS, PROMAX_WHITELIST,
    PUBLIC_DOH_BOOTSTRAPS, is_promax_domain, read_public, read_public_bounded,
    validate_native_rewrites, validate_promax_load_budget,
};
use crate::promax::rule::SurgeRule;
use crate::promax::safety::{SafetyDecision, SafetyPolicy, classify_domain_payload};
use crate::promax::source::{DownloadConfig, Downloader};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

mod platforms;

const HEIDAI_FULL: &str =
    "https://github.com/217heidai/adblockfilters/releases/download/rule/adblockdns.txt";
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const LOCAL_MARKER: &str = "# Script Hub local export · 217heidai complete input";
const PREVIEW_FILE: &str = "ScriptHub.AdBlock.Heidai.Full.FlClash.sgmodule";
const SINGBOX_SOURCE: &str = "ScriptHub.AdBlock.Heidai.Full.SingBox.json";
const SINGBOX_BINARY: &str = "ScriptHub.AdBlock.Heidai.Full.SingBox.srs";
const LOCAL_GUARDS: &[&str] = &[
    "DOMAIN-SUFFIX,apple.com",
    "DOMAIN-SUFFIX,aaplimg.com",
    "DOMAIN,apple-relay.cloudflare.com",
    "DOMAIN,apple-relay.fastly-edge.com",
    "DOMAIN,126.cn",
    "DOMAIN,123cha.com",
    "DOMAIN,0575bbs.com",
];

struct Feed {
    blocks: BTreeSet<String>,
    block_rows: usize,
    unsupported_blocks: usize,
    exceptions: HashSet<String>,
    exception_rows: usize,
    version: String,
}

fn parse_feed(text: &str) -> Result<Feed, String> {
    let mut blocks = BTreeSet::new();
    let mut exceptions = HashSet::new();
    let mut version = None;
    let mut block_count = None;
    let mut exception_count = None;
    let mut block_rows = 0;
    let mut exception_rows = 0;
    let mut unsupported_blocks = 0;
    for (index, raw) in text.lines().enumerate() {
        let row = raw.trim().trim_start_matches('\u{feff}');
        if let Some(value) = row.strip_prefix("! Version: ") {
            version = Some(value);
        } else if let Some(value) = row.strip_prefix("! Blocked domains: ") {
            block_count = value.parse::<usize>().ok();
        } else if let Some(value) = row.strip_prefix("! unBlocked domains: ") {
            exception_count = value.parse::<usize>().ok();
        } else if row.starts_with("!#") {
            return Err("Complete source contains an unsupported conditional".into());
        } else if !row.is_empty() && !row.starts_with('!') && row != "[Adblock Plus 2.0]" {
            let (exception, pattern) = row
                .strip_prefix("@@")
                .map_or((false, row), |pattern| (true, pattern));
            let anchor = pattern.strip_prefix("||").ok_or_else(|| {
                format!("Unsupported complete-source record on line {}", index + 1)
            })?;
            let host = anchor
                .strip_suffix('^')
                .map(str::to_ascii_lowercase)
                .filter(|host| is_promax_domain(host));
            if exception {
                exception_rows += 1;
                exceptions.insert(host.ok_or_else(|| {
                    format!(
                        "Unsupported complete-source exception on line {}",
                        index + 1
                    )
                })?);
            } else {
                block_rows += 1;
                if let Some(host) = host {
                    blocks.insert(host);
                } else {
                    unsupported_blocks += 1;
                }
            }
        }
    }
    let version = version
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or("Complete source is missing a valid version")?;
    if blocks.is_empty()
        || block_count != Some(block_rows)
        || exception_count != Some(exception_rows)
    {
        return Err("Complete source counts do not match its release header".into());
    }
    Ok(Feed {
        blocks,
        block_rows,
        unsupported_blocks,
        exceptions,
        exception_rows,
        version: version.into(),
    })
}

fn ancestors(host: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(host), |name| {
        name.split_once('.').map(|(_, tail)| tail)
    })
}

fn select_domains<'a>(
    base: &str,
    feed: &'a Feed,
    safety: &SafetyPolicy,
) -> Result<Vec<&'a str>, String> {
    let exception_parents = feed
        .exceptions
        .iter()
        .flat_map(|host| ancestors(host))
        .collect::<HashSet<_>>();
    let mitm_hosts = base
        .split_once("\n[MITM]\n")
        .into_iter()
        .flat_map(|(_, section)| section.lines())
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.strip_prefix("hostname = %APPEND% "))
        .flat_map(|hosts| hosts.split(',').map(str::trim))
        .filter(|host| !host.starts_with('-') && is_promax_domain(host));
    let infrastructure_parents = mitm_hosts
        .chain(PUBLIC_DOH_BOOTSTRAPS.iter().map(|&(host, _)| host))
        .flat_map(ancestors)
        .collect::<HashSet<_>>();
    let mut selected = BTreeSet::new();
    for host in &feed.blocks {
        if ancestors(host).any(|name| feed.exceptions.contains(name))
            || exception_parents.contains(host.as_str())
            || infrastructure_parents.contains(host.as_str())
            || psl::suffix(host.as_bytes())
                .is_some_and(|suffix| suffix.is_known() && suffix.as_bytes() == host.as_bytes())
        {
            continue;
        }
        let line = format!("DOMAIN-SUFFIX,{host},REJECT");
        let rule = SurgeRule::parse(&line).map_err(|error| error.to_string())?;
        if safety.decision(&rule) == SafetyDecision::Keep
            && classify_domain_payload(host) == SafetyDecision::Keep
        {
            selected.insert(host.as_str());
        }
    }
    Ok(selected
        .iter()
        .copied()
        .filter(|host| {
            !ancestors(host)
                .skip(1)
                .any(|parent| selected.contains(parent))
        })
        .collect())
}

fn append_full_feed(
    base: &str,
    version: &str,
    domains: &[&str],
) -> Result<(String, usize), String> {
    if !base.lines().any(|line| line == OWNER) {
        return Err("Local export requires the repository-generated FlClash PROMAX".into());
    }
    let rule_start = base.find("\n[Rule]\n").ok_or("Missing PROMAX rules")? + "\n[Rule]\n".len();
    let rule_end = base[rule_start..]
        .find("\n[")
        .map_or(base.len(), |index| rule_start + index);
    let rules = &base[rule_start..rule_end];
    let suffixes = rules
        .lines()
        .filter_map(|row| {
            let (host, policy) = row.strip_prefix("DOMAIN-SUFFIX,")?.split_once(',')?;
            super::is_block_policy(policy).then_some(host)
        })
        .collect::<HashSet<_>>();
    let mut additions = String::new();
    let mut count = 0;
    for host in domains
        .iter()
        .filter(|host| !ancestors(host).any(|name| suffixes.contains(name)))
    {
        additions.push_str("DOMAIN-SUFFIX,");
        additions.push_str(host);
        additions.push_str(",REJECT\n");
        count += 1;
    }
    let (_, rule_prefix) = base[..rule_start]
        .split_once('\n')
        .ok_or("Missing PROMAX metadata")?;
    let header = format!(
        "#!name=FlClash Preview · AdBlock PROMAX (217heidai Full Local)\n{LOCAL_MARKER}\n# Complete upstream: {HEIDAI_FULL}\n# Upstream version: {version}\n"
    );
    let mut result = String::with_capacity(base.len() + additions.len() + header.len());
    result.push_str(&header);
    result.push_str(rule_prefix);
    result.push_str(rules);
    if !rules.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(&additions);
    result.push_str(&base[rule_end..]);
    let active = rules
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .count()
        + count
        + base[rule_end..]
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with(['#', '[']))
            .count();
    validate_promax_load_budget(result.len(), active)?;
    validate_native_rewrites(&result)?;
    Ok((result, count))
}

pub fn export_heidai_full(root: &Path, output: &Path) -> Result<String, String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let name = output.file_name().ok_or("Specify an output file")?;
    let output = parent.join(name);
    if parent.starts_with(&root) || output.extension() != Some(OsStr::new("sgmodule")) {
        return Err("Choose a local .sgmodule file outside the repository".into());
    }
    match fs::symlink_metadata(&output) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            let mut header = Vec::new();
            fs::File::open(&output)
                .and_then(|file| file.take(512).read_to_end(&mut header))
                .map_err(|error| error.to_string())?;
            if !String::from_utf8_lossy(&header)
                .lines()
                .any(|line| line == LOCAL_MARKER)
            {
                return Err("Output exists and is not a Script Hub complete-source export".into());
            }
        }
        Ok(_) => return Err("Output must be a regular file".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    let (base, feed, safety, digest) = load_inputs(&root)?;
    let domains = select_domains(&base, &feed, &safety)?;
    let (result, added) = append_full_feed(&base, &feed.version, &domains)?;
    crate::filesystem::replace_file(&output, result.as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "{} · {added} additional domains · {} bytes\n{}",
        source_report(&feed, &digest),
        result.len(),
        output.display()
    ))
}

fn load_inputs(root: &Path) -> Result<(String, Feed, SafetyPolicy, String), String> {
    let base = read_public_bounded(root, &root.join(MAIN_MODULE), MAX_PROMAX_BYTES)?;
    let whitelist = read_public(root, &root.join(PROMAX_WHITELIST))?;
    let direct = read_public(root, &root.join("rulesets/RULE-SET/Direct.list"))?;
    let safety =
        SafetyPolicy::from_lines(
            whitelist
                .lines()
                .chain(direct.lines().filter(|line| {
                    line.starts_with("DOMAIN,") || line.starts_with("DOMAIN-SUFFIX,")
                }))
                .chain(
                    PROMAX_PROTECTED_SERVICE_HOSTS
                        .iter()
                        .chain(LOCAL_GUARDS)
                        .copied(),
                ),
        );
    let raw = Downloader::new(DownloadConfig {
        max_bytes: MAX_SOURCE_BYTES as u64,
        ..DownloadConfig::default()
    })
    .get(HEIDAI_FULL)
    .map_err(|error| error.to_string())?;
    let feed = parse_feed(&raw)?;
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(raw.as_bytes()) {
        write!(digest, "{byte:02x}").expect("writing to a string cannot fail");
    }
    Ok((base, feed, safety, digest))
}

fn source_report(feed: &Feed, digest: &str) -> String {
    format!(
        "217heidai {} · {} blocks read · {} exceptions read · {} unsupported blocks skipped · upstream SHA-256 {digest}",
        feed.version, feed.block_rows, feed.exception_rows, feed.unsupported_blocks,
    )
}

/// Export one protected full-source snapshot in the native formats the project supports.
pub fn export_heidai_bundle(
    root: &Path,
    directory: &Path,
    singbox: Option<&Path>,
) -> Result<String, String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let parent = directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let name = directory
        .file_name()
        .ok_or("Specify a new output directory")?;
    if parent.starts_with(&root) {
        return Err("Choose a new local directory outside the repository".into());
    }
    let directory = parent.join(name);
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
        Ok(_) => {
            return Err("Choose a new output directory; the destination already exists".into());
        }
    }
    let (base, feed, safety, digest) = load_inputs(&root)?;
    let domains = select_domains(&base, &feed, &safety)?;
    let (flclash, added) = append_full_feed(&base, &feed.version, &domains)?;
    let mut files = platforms::render(&domains)?;
    files.push((PREVIEW_FILE, flclash));
    let names: Vec<_> = files
        .iter()
        .map(|(name, _)| *name)
        .chain(singbox.map(|_| SINGBOX_BINARY))
        .collect();
    let manifest = serde_json::to_string_pretty(&serde_json::json!({
        "source": HEIDAI_FULL,
        "version": feed.version,
        "upstream_sha256": digest,
        "source_block_rows": feed.block_rows,
        "source_exception_rows": feed.exception_rows,
        "unsupported_block_rows": feed.unsupported_blocks,
        "protected_full_source_matchers": domains.len(),
        "additional_flclash_matchers": added,
        "files": names,
    }))
    .map_err(|error| error.to_string())?;

    let report = format!(
        "{}\n{} protected full-source matchers · {added} additional FlClash matchers\n{}",
        source_report(&feed, &digest),
        domains.len(),
        directory.display()
    );
    fs::create_dir(&directory).map_err(|error| error.to_string())?;
    let mut written = Vec::new();
    let result = (|| -> Result<(), String> {
        for (name, content) in files {
            let path = directory.join(name);
            crate::filesystem::replace_file(&path, content.as_bytes())
                .map_err(|error| format!("cannot write {name}: {error}"))?;
            written.push(path);
        }
        if let Some(singbox) = singbox {
            let binary = directory.join(SINGBOX_BINARY);
            written.push(binary.clone());
            let process = crate::pipeline::srs_generator::compiler_command(
                &singbox.to_string_lossy(),
                &directory.join(SINGBOX_SOURCE),
                &binary,
            )
            .output()
            .map_err(|error| format!("cannot run sing-box: {error}"))?;
            if !process.status.success() {
                return Err(format!(
                    "sing-box could not compile protected feed: {}",
                    String::from_utf8_lossy(&process.stderr).trim()
                ));
            }
            let mut header = [0; 4];
            if fs::File::open(&binary)
                .and_then(|mut file| file.read_exact(&mut header))
                .is_err()
                || !header.starts_with(b"SRS")
            {
                return Err("sing-box did not produce a binary ruleset".into());
            }
        }
        crate::filesystem::replace_file(
            &directory.join("manifest.json"),
            format!("{manifest}\n").as_bytes(),
        )
        .map_err(|error| format!("cannot write bundle manifest: {error}"))?;
        Ok(())
    })();
    if result.is_err() {
        for path in written {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_dir(&directory);
    }
    result?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_declared_counts_and_does_not_widen_abp_conditions() {
        let header = "! Version: 20261118144847\n! Blocked domains: 2\n! unBlocked domains: 1\n";
        let text = format!(
            "{header}||ordinary.test^\n||fb_servpub-a.example.com^\n@@||child.ordinary.test^\n"
        );
        let feed = parse_feed(&text).unwrap();
        assert_eq!(feed.blocks.len(), 2);
        assert_eq!(feed.exceptions.len(), 1);
        assert!(parse_feed(&text.replace("2\n!", "3\n!")).is_err());
        let scoped =
            parse_feed(&text.replace("||ordinary.test^", "||ordinary.test^$third-party")).unwrap();
        assert_eq!(scoped.block_rows, 2);
        assert_eq!(scoped.unsupported_blocks, 1);
        assert!(!scoped.blocks.contains("ordinary.test"));
        assert!(
            parse_feed(&text.replace("child.ordinary.test^", "child.ordinary.test^$third-party"))
                .is_err()
        );
    }

    #[test]
    fn keeps_source_exceptions_apple_and_base_routing() {
        let text = "! Version: 20261118144847\n! Blocked domains: 9\n! unBlocked domains: 1\n\
                    ||ad.test^\n||copy.base.test^\n||safe.test^\n||private.child.safe.test^\n\
                    ||doh.dns.apple.com^\n||dup.ad.test^\n||media.test^\n\
                    ||dns.google^\n||co.uk^\n@@||child.safe.test^\n";
        let base = format!(
            "#!name=PROMAX\n{OWNER}\n[Rule]\nDOMAIN-SUFFIX,base.test,REJECT\n\n[MITM]\nhostname = %APPEND% playback.media.test\n"
        );
        let feed = parse_feed(text).unwrap();
        let domains = select_domains(
            &base,
            &feed,
            &SafetyPolicy::from_lines(LOCAL_GUARDS.iter().copied()),
        )
        .unwrap();
        assert_eq!(domains, ["ad.test", "copy.base.test"]);
        let (output, count) = append_full_feed(&base, &feed.version, &domains).unwrap();
        assert_eq!(count, 1);
        assert!(output.contains("DOMAIN-SUFFIX,ad.test,REJECT\n"));
        assert!(output.contains("DOMAIN-SUFFIX,base.test,REJECT\n"));
        assert!(output.ends_with("hostname = %APPEND% playback.media.test\n"));
        assert!(!output.contains("DOMAIN-SUFFIX,media.test,REJECT\n"));
        assert!(!output.contains("DOMAIN-SUFFIX,dns.google,REJECT\n"));
        assert!(!output.contains("DOMAIN-SUFFIX,co.uk,REJECT\n"));
        assert!(!output.contains("safe.test") && !output.contains("doh.dns.apple.com"));
    }
}
