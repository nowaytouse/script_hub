//! Local cross-client packaging for one verified full-source snapshot.

use super::firewall_sync::PUBLIC_DOH_BOOTSTRAPS;
use super::flclash::PROMAX_PROTECTED_SERVICE_HOSTS;
use super::flclash::full_feed::{LOCAL_MARKER, append_full_feed, load_base, positive_mitm_hosts};
use crate::filesystem::read_public;
use crate::promax::heidai_feed::{self, Feed, HEIDAI_FULL, select_domains};
use crate::promax::safety::{SafetyPolicy, WHITELIST_PATH};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

mod platforms;

const PREVIEW_FILE: &str = "ScriptHub.AdBlock.Heidai.Full.FlClash.sgmodule";
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

fn protected_hosts(base: &str) -> impl Iterator<Item = &str> {
    positive_mitm_hosts(base).chain(PUBLIC_DOH_BOOTSTRAPS.iter().map(|&(host, _)| host))
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
    let domains = select_domains(&feed, &safety, protected_hosts(&base))?;
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
    let base = load_base(root)?;
    let whitelist = read_public(root, &root.join(WHITELIST_PATH))?;
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
    let (feed, digest) = heidai_feed::download()?;
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
    let domains = select_domains(&feed, &safety, protected_hosts(&base))?;
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
    fn keeps_source_exceptions_apple_and_base_routing() {
        let text = "! Version: 20261118144847\n! Blocked domains: 9\n! unBlocked domains: 1\n\
                    ||ad.test^\n||copy.base.test^\n||safe.test^\n||private.child.safe.test^\n\
                    ||doh.dns.apple.com^\n||dup.ad.test^\n||media.test^\n\
                    ||dns.google^\n||co.uk^\n@@||child.safe.test^\n";
        let base = "#!name=PROMAX\n# Generated by Script Hub FlClash preview generator\n[Rule]\nDOMAIN-SUFFIX,base.test,REJECT\n\n[MITM]\nhostname = %APPEND% playback.media.test\n";
        let feed = parse_feed(text).unwrap();
        let domains = select_domains(
            &feed,
            &SafetyPolicy::from_lines(LOCAL_GUARDS.iter().copied()),
            protected_hosts(base),
        )
        .unwrap();
        assert_eq!(domains, ["ad.test", "copy.base.test"]);
        let (output, count) = append_full_feed(base, &feed.version, &domains).unwrap();
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
