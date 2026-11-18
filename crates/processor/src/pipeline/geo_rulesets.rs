//! Maintain the country inputs for the existing sing-box rulesets.

mod mmdb;
pub(super) mod model;

use crate::promax::artifact::{ArtifactKind, GeneratedArtifact, publish_generated_artifacts};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

pub(crate) const COUNTRY_INPUTS: &str = "rulesets/Sources/compiled/geoip";
pub(crate) const COUNTRIES: &[&str] = &["CN", "JP", "KR", "TW", "US"];
pub(crate) const MMDB_URL: &str =
    "https://github.com/daijro/geoip-all-in-one/releases/latest/download/geoip-aio-all.mmdb";

pub(crate) fn refresh_or_keep_countries(root: &Path) -> Result<(), String> {
    if let Err(error) = refresh_countries(root) {
        if !COUNTRIES.iter().all(|country| {
            root.join(COUNTRY_INPUTS)
                .join(format!("GeoIP_{country}.list"))
                .is_file()
        }) {
            return Err(error);
        }
        eprintln!("[WARN] Keeping the last AIO country snapshots: {error}");
    }
    Ok(())
}

fn refresh_countries(root: &Path) -> Result<(), String> {
    // Keep the source database in memory. Only selected country ranges are published.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(120)))
        .max_redirects(5)
        .user_agent("ScriptHub GeoIP country refresh")
        .build()
        .into();
    let mut response = agent
        .get(MMDB_URL)
        .call()
        .map_err(|_| "AIO MMDB download failed")?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(mmdb::MAX_DB_BYTES as u64)
        .read_to_vec()
        .map_err(|_| "AIO MMDB download failed or exceeded its size limit")?;
    let countries: Vec<String> = COUNTRIES.iter().map(|code| (*code).to_owned()).collect();
    let extracted = mmdb::extract(&bytes, &countries)?;
    if extracted.database_type != "GeoIP2-City" || extracted.ip_version != 6 {
        return Err("Expected the combined IPv4/IPv6 AIO country database".into());
    }
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(&bytes) {
        write!(digest, "{byte:02x}").expect("writing to a string");
    }
    let mut outputs = Vec::new();
    for (country, cidrs) in extracted.countries {
        let name = format!("GeoIP_{country}");
        let mut content = crate::generate_header(
            &name,
            "AIO country snapshot for the existing sing-box country fallback",
            cidrs.len(),
        );
        writeln!(
            content,
            "# Source: {MMDB_URL}\n# Database build epoch: {}\n# Database SHA256: {digest}",
            extracted.build_epoch
        )
        .expect("writing to a string");
        if !cidrs.iter().any(|cidr| cidr.contains(':'))
            || !cidrs.iter().any(|cidr| !cidr.contains(':'))
        {
            return Err(format!(
                "AIO country {country} is missing one address family"
            ));
        }
        for cidr in &cidrs {
            let kind = if cidr.contains(':') {
                "IP-CIDR6"
            } else {
                "IP-CIDR"
            };
            writeln!(content, "{kind},{cidr}").expect("writing to a string");
        }
        println!(
            "[INFO] {name}: {} CIDRs from AIO build {}",
            cidrs.len(),
            extracted.build_epoch
        );
        outputs.push(GeneratedArtifact {
            target: root.join(COUNTRY_INPUTS).join(format!("{name}.list")),
            content,
            kind: ArtifactKind::ExternalRuleset,
        });
    }
    publish_generated_artifacts(root, &outputs)
}
