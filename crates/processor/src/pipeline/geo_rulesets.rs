//! Maintain the country inputs for the existing sing-box rulesets.

mod mmdb;
pub(super) mod model;
mod project;
mod release;

use crate::promax::artifact::{ArtifactKind, GeneratedArtifact, publish_generated_artifacts};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::Path;

pub(crate) const COUNTRY_INPUTS: &str = "rulesets/Sources/compiled/geoip";
pub(crate) const COUNTRIES: &[&str] = &["CN", "JP", "KR", "TW", "US"];
pub(crate) fn validate_databases(root: &Path) -> Result<(), String> {
    release::validate(root)
}
pub(crate) const MMDB_URL: &str =
    "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/GeoData/Country.mmdb";
#[cfg(feature = "gui")]
pub(crate) const ASN_URL: &str =
    "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/GeoData/ASN.mmdb";

pub(crate) fn refresh_or_keep_countries(root: &Path) -> Result<(), String> {
    let bytes = release::refresh(root)?;
    let countries: Vec<String> = COUNTRIES.iter().map(|code| (*code).to_owned()).collect();
    let extracted = mmdb::extract(&bytes, &countries)?;
    if extracted.database_type != "GeoLite2-Country" || extracted.ip_version != 6 {
        return Err("Expected the converted IPv4/IPv6 Country database".into());
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
            "ipgeo Community Edition country snapshot for the sing-box country fallback",
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
                "ipgeo country {country} is missing one address family"
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
            "[INFO] {name}: {} CIDRs from ipgeo build {}",
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

pub(crate) fn update_country_assets(root: &Path, singbox: &Path) -> Result<(), String> {
    refresh_or_keep_countries(root)?;
    let binary = singbox.to_str().ok_or("Invalid sing-box executable path")?;
    for code in COUNTRIES {
        let list = root.join(COUNTRY_INPUTS).join(format!("GeoIP_{code}.list"));
        if !super::srs_generator::compile_srs(&list, root, binary, false) {
            return Err(format!("Failed to compile country rules for {code}"));
        }
    }
    Ok(())
}
