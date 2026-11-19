//! Local cross-client packaging for one verified full-source snapshot.

use super::firewall_sync::PUBLIC_DOH_BOOTSTRAPS;
use super::merge_bundles::hostname_tokens;
use crate::dns_name::is_dotted_ascii_host;
use crate::filesystem::read_public;
use crate::promax::SURGE_PROMAX_PATH;
use crate::promax::heidai_feed::{self, Feed, HEIDAI_FULL, select_domains};
use crate::promax::safety::{SafetyPolicy, WHITELIST_PATH};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

mod platforms;

const SINGBOX_BINARY: &str = "ScriptHub.AdBlock.Heidai.Full.SingBox.srs";
const LOCAL_GUARDS: &[&str] = &[
    "DOMAIN-SUFFIX,apple.com",
    "DOMAIN-SUFFIX,aaplimg.com",
    "DOMAIN,apple-relay.cloudflare.com",
    "DOMAIN,apple-relay.fastly-edge.com",
    "DOMAIN,126.cn",
    "DOMAIN,123cha.com",
    "DOMAIN,0575bbs.com",
    "DOMAIN,17u.cn",
    "DOMAIN,www.17u.cn",
    "DOMAIN,wx.17u.cn",
    "DOMAIN,mada-travel.17u.cn",
    "DOMAIN,mo.baidu.com",
];

fn mitm_guard_lines(module: &str) -> impl Iterator<Item = String> + '_ {
    module
        .split_once("\n[MITM]\n")
        .into_iter()
        .flat_map(|(_, section)| section.lines())
        .take_while(|line| !line.trim_start().starts_with('['))
        .map(str::trim)
        .filter(|line| {
            line.split_once('=')
                .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("hostname"))
        })
        .flat_map(hostname_tokens)
        .filter_map(|token| {
            let host = token.split(':').next()?;
            let (kind, host) = host
                .strip_prefix("*.")
                .map_or(("DOMAIN", host), |suffix| ("DOMAIN-SUFFIX", suffix));
            (!host.starts_with('-') && is_dotted_ascii_host(host)).then(|| format!("{kind},{host}"))
        })
}

fn feed_safety(module: &str, whitelist: &str, direct: &str) -> SafetyPolicy {
    let shared_hosts: Vec<_> = mitm_guard_lines(module)
        .chain(
            PUBLIC_DOH_BOOTSTRAPS
                .iter()
                .map(|&(host, _)| format!("DOMAIN,{host}")),
        )
        .collect();
    SafetyPolicy::from_lines(
        whitelist
            .lines()
            .chain(
                direct.lines().filter(|line| {
                    line.starts_with("DOMAIN,") || line.starts_with("DOMAIN-SUFFIX,")
                }),
            )
            .chain(LOCAL_GUARDS.iter().copied())
            .chain(shared_hosts.iter().map(String::as_str)),
    )
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
    let module = read_public(&root, &root.join(SURGE_PROMAX_PATH))?;
    let whitelist = read_public(&root, &root.join(WHITELIST_PATH))?;
    let direct = read_public(&root, &root.join("rulesets/RULE-SET/Direct.list"))?;
    let safety = feed_safety(&module, &whitelist, &direct);
    let (feed, digest) = heidai_feed::download()?;
    let domains = select_domains(&feed, &safety)?;
    let files = platforms::render(&domains)?;
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
        "files": names,
    }))
    .map_err(|error| error.to_string())?;

    let report = format!(
        "{}\n{} protected full-source matchers\n{}",
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
                &directory.join(platforms::SING_BOX),
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
    use crate::promax::heidai_feed::parse_feed;

    #[test]
    fn keeps_source_exceptions_and_shared_surge_mitm_scopes() {
        let text = "! Version: 20261118144847\n! Blocked domains: 11\n! unBlocked domains: 1\n\
                    ||ad.test^\n||safe.test^\n||private.child.safe.test^\n||doh.dns.apple.com^\n\
                    ||dup.ad.test^\n||media.test^\n||leaf.rewrite.test^\n||dns.google^\n||co.uk^\n\
                    ||excluded.test^\n||direct.test^\n@@||child.safe.test^\n";
        let module = "#!name=PROMAX\n\n[MITM]\nhostname = %APPEND% playback.media.test:443, *.rewrite.test, -excluded.test\n";
        let feed = parse_feed(text).unwrap();
        let safety = feed_safety(module, "", "DOMAIN,api.direct.test");
        assert_eq!(
            select_domains(&feed, &safety).unwrap(),
            ["ad.test", "excluded.test"]
        );
    }
}
