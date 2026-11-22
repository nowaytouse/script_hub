use super::project;
use crate::filesystem::replace_file;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path, time::Duration};

pub(super) const DIRECTORY: &str = "rulesets/GeoData";
const BASE: &str = "https://github.com/whois-api-llc/ipgeo-community/releases";
const INPUT: &str = "ipgeo-community.mmdb";
const OUTPUTS: [&str; 4] = ["Country.mmdb", "ASN.mmdb", "LICENSE.txt", "ATTRIBUTION.txt"];
const MAX_OUTPUT: usize = 64 << 20;
const MAX_NOTICE: usize = 64 << 10;

#[derive(Deserialize)]
struct Manifest {
    version: String,
    license: String,
    files: BTreeMap<String, Asset>,
}
#[derive(Deserialize)]
struct Asset {
    bytes: usize,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    projection_version: u8,
    version: String,
    upstream_sha256: String,
    files: BTreeMap<String, String>,
}

fn sha(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(hex, "{byte:02x}").expect("writing to a string");
    }
    hex
}

fn parse_manifest(bytes: &[u8]) -> Result<(String, Asset), String> {
    let mut manifest: Manifest = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let asset = manifest
        .files
        .remove(INPUT)
        .ok_or("ipgeo release lacks the MMDB")?;
    if manifest.license != "CC BY-SA 4.0"
        || !chrono::NaiveDate::parse_from_str(&manifest.version, "%Y-%m-%d")
            .is_ok_and(|date| date.to_string() == manifest.version)
        || !(1..=super::mmdb::MAX_DB_BYTES).contains(&asset.bytes)
        || asset.sha256.len() != 64
        || !asset
            .sha256
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err("Unexpected ipgeo release version, license, size or digest".into());
    }
    Ok((manifest.version, asset))
}

fn fetch(agent: &ureq::Agent, url: &str, limit: usize) -> Result<Vec<u8>, String> {
    agent
        .get(url)
        .call()
        .map_err(|e| format!("ipgeo download failed: {e}"))?
        .body_mut()
        .with_config()
        .limit(limit as u64)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

fn local(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > limit as u64 {
        return Err(format!("Invalid geo artifact: {}", path.display()));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    fs::File::open(path)
        .and_then(|f| f.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Geo artifact exceeded its limit".into());
    }
    Ok(bytes)
}

fn verified(directory: &Path) -> Result<(Receipt, Vec<u8>), String> {
    let receipt: Receipt =
        serde_json::from_slice(&local(&directory.join("release.json"), MAX_NOTICE)?)
            .map_err(|e| e.to_string())?;
    if receipt.projection_version != 1 || receipt.files.len() != OUTPUTS.len() {
        return Err("Stale ipgeo projection".into());
    }
    let mut country = Vec::new();
    for name in OUTPUTS {
        let bytes = local(
            &directory.join(name),
            if name.ends_with(".txt") {
                MAX_NOTICE
            } else {
                MAX_OUTPUT
            },
        )?;
        if receipt.files.get(name) != Some(&sha(&bytes)) {
            return Err(format!("Geo artifact digest mismatch: {name}"));
        }
        if name.ends_with(".mmdb") {
            let db = maxminddb::Reader::from_source(&bytes).map_err(|e| e.to_string())?;
            let expected = if name == OUTPUTS[0] {
                "GeoLite2-Country"
            } else {
                "GeoLite2-ASN"
            };
            if db.metadata().database_type != expected || db.metadata().ip_version != 6 {
                return Err(format!("Wrong geo database type or address family: {name}"));
            }
        }
        if name == OUTPUTS[0] {
            country = bytes;
        }
    }
    Ok((receipt, country))
}

// The schema writer is deterministic except for the source build epoch. Keep an
// unchanged routing table byte-for-byte even if a weekly release only touched city data.
fn equivalent_projection(old: &[u8], new: &[u8]) -> bool {
    const MARKER: &[u8] = b"\xab\xcd\xefMaxMind.com";
    fn body(bytes: &[u8]) -> Option<&[u8]> {
        let start = bytes.len().saturating_sub(128 << 10);
        let offset = bytes[start..]
            .windows(MARKER.len())
            .rposition(|s| s == MARKER)?;
        Some(&bytes[..start + offset])
    }
    if body(old).zip(body(new)).is_none_or(|(a, b)| a != b) {
        return false;
    }
    let (Ok(a), Ok(b)) = (
        maxminddb::Reader::from_source(old),
        maxminddb::Reader::from_source(new),
    ) else {
        return false;
    };
    let mut meta = a.metadata().clone();
    meta.build_epoch = b.metadata().build_epoch;
    &meta == b.metadata()
}

fn keep_equivalent(path: &Path, new: &mut Vec<u8>) {
    if let Ok(old) = local(path, MAX_OUTPUT)
        && equivalent_projection(&old, new)
    {
        *new = old;
    }
}

pub(super) fn validate(root: &Path) -> Result<(), String> {
    verified(&root.join(DIRECTORY)).map(|_| ())
}

pub(super) fn refresh(root: &Path) -> Result<Vec<u8>, String> {
    let dir = root.join(DIRECTORY);
    for path in [root.join("rulesets"), dir.clone()] {
        if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
            return Err(format!("Invalid GeoData directory: {}", path.display()));
        }
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(240)))
        .max_redirects(5)
        .user_agent("ScriptHub ipgeo update")
        .build()
        .into();
    let previous = verified(&dir);
    let manifest = match fetch(
        &agent,
        &format!("{BASE}/latest/download/MANIFEST.json"),
        MAX_NOTICE,
    ) {
        Ok(manifest) => manifest,
        Err(e) => {
            if let Ok((receipt, country)) = previous {
                eprintln!("[WARN] Keeping verified ipgeo {}: {e}", receipt.version);
                return Ok(country);
            }
            return Err(e);
        }
    };
    let (version, asset) = parse_manifest(&manifest)?;
    if let Ok((receipt, country)) = previous {
        if receipt.version > version {
            return Err("ipgeo latest manifest predates the verified local release".into());
        }
        if receipt.version == version && receipt.upstream_sha256 == asset.sha256 {
            println!("[INFO] ipgeo {version} verified and unchanged; large download skipped");
            return Ok(country);
        }
    }
    let pinned = format!("{BASE}/download/community-{version}");
    let input = fetch(&agent, &format!("{pinned}/{INPUT}"), asset.bytes)?;
    if input.len() != asset.bytes || sha(&input) != asset.sha256 {
        return Err("ipgeo release MMDB length or SHA-256 mismatch".into());
    }
    let license = fetch(&agent, &format!("{pinned}/LICENSE"), MAX_NOTICE)?;
    let attribution = fetch(&agent, &format!("{pinned}/ATTRIBUTION.txt"), MAX_NOTICE)?;
    if !std::str::from_utf8(&license)
        .is_ok_and(|s| s.contains("https://creativecommons.org/licenses/by-sa/4.0/"))
        || !std::str::from_utf8(&attribution)
            .is_ok_and(|s| s.contains("WhoisXML") && s.contains("DB-IP"))
    {
        return Err("Missing ipgeo release license or attribution".into());
    }
    let mut output = project::convert(&input)?;
    drop(input);
    keep_equivalent(&dir.join(OUTPUTS[0]), &mut output.country);
    keep_equivalent(&dir.join(OUTPUTS[1]), &mut output.asn);
    let entries = [
        output.country.as_slice(),
        &output.asn,
        &license,
        &attribution,
    ];
    if entries.iter().any(|b| b.is_empty() || b.len() > MAX_OUTPUT) {
        return Err("Empty or oversized converted geo artifact".into());
    }
    let receipt = Receipt {
        projection_version: 1,
        version,
        upstream_sha256: asset.sha256,
        files: OUTPUTS
            .into_iter()
            .zip(entries)
            .map(|(name, bytes)| (name.into(), sha(bytes)))
            .collect(),
    };
    let mut json = serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?;
    json.push(b'\n');
    for (name, bytes) in OUTPUTS
        .into_iter()
        .zip(entries)
        .chain([("release.json", json.as_slice())])
    {
        if local(&dir.join(name), MAX_OUTPUT).is_ok_and(|old| old == bytes) {
            continue;
        }
        replace_file(&dir.join(name), bytes).map_err(|e| e.to_string())?;
    }
    Ok(output.country)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn build_epoch_alone_does_not_republish_a_database() {
        let sample = |seconds, name: &str, value: &str| {
            let mut w = mmdb_writer::Writer::builder(name)
                .build_epoch(std::time::UNIX_EPOCH + Duration::from_secs(seconds))
                .build();
            w.insert("192.0.2.0/24".parse::<ipnet::IpNet>().unwrap(), value)
                .unwrap();
            w.to_bytes().unwrap()
        };
        let first = sample(10, "test", "A");
        assert!(equivalent_projection(&first, &sample(20, "test", "A")));
        assert!(!equivalent_projection(&first, &sample(20, "test", "B")));
        assert!(!equivalent_projection(&first, &sample(20, "other", "A")));
        assert!(!equivalent_projection(&first, b"missing"));
    }
    #[test]
    fn only_supported_pinned_releases_are_downloadable() {
        let digest = "a".repeat(64);
        let manifest = serde_json::json!({"version":"2026-11-20","license":"CC BY-SA 4.0","files":{(INPUT):{"bytes":1024,"sha256":digest}}});
        assert!(parse_manifest(&serde_json::to_vec(&manifest).unwrap()).is_ok());
        for (pointer, value) in [
            ("/version", serde_json::json!("../latest")),
            ("/license", serde_json::json!("private")),
            (
                "/files/ipgeo-community.mmdb/sha256",
                serde_json::json!("wrong"),
            ),
            (
                "/files/ipgeo-community.mmdb/bytes",
                serde_json::json!(super::super::mmdb::MAX_DB_BYTES + 1),
            ),
        ] {
            let mut bad = manifest.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(parse_manifest(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
    }
}
