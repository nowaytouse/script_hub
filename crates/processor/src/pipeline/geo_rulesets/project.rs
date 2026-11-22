//! Project the upstream flat schema into two independent, typed MMDBs.

use maxminddb::{Reader, WithinOptions, geoip2};
use mmdb_writer::{Ipv4Aliasing, MetadataPointers, Writer};
use serde::Deserialize;
use std::time::{Duration, UNIX_EPOCH};

#[derive(Deserialize, Default)]
struct Flat<'a> {
    country_code: Option<&'a str>,
    asn: Option<u32>,
    as_org: Option<&'a str>,
}

pub(super) struct Databases {
    pub country: Vec<u8>,
    pub asn: Vec<u8>,
}

pub(super) fn convert(bytes: &[u8]) -> Result<Databases, String> {
    let reader = Reader::from_source(bytes).map_err(|e| e.to_string())?;
    let meta = reader.metadata();
    if meta.database_type != "ipgeo" || meta.ip_version != 6 || meta.node_count > 32_000_000 {
        return Err("Expected an ipgeo IPv4/IPv6 MMDB".into());
    }
    let epoch = UNIX_EPOCH
        .checked_add(Duration::from_secs(meta.build_epoch))
        .ok_or("Invalid ipgeo build epoch")?;
    let writer = |name| {
        Writer::builder(name)
            .build_epoch(epoch)
            .description(&[("en", "Script Hub / ipgeo Community Edition (CC BY-SA 4.0)")])
            .ipv4_aliasing(Ipv4Aliasing::Disabled)
            .metadata_pointers(MetadataPointers::Disabled)
            .build()
    };
    let mut country = writer("GeoLite2-Country");
    let mut asn = writer("GeoLite2-ASN");
    let mut seen = [[0_usize; 2]; 2];
    let mut samples = Vec::new();
    // Preserve canonical networks, not synthetic IPv4 aliases inside IPv6 space.
    let options = WithinOptions::default().include_networks_without_data();
    for (index, item) in reader
        .networks(options)
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        if index >= 32_000_000 {
            return Err("ipgeo network traversal exceeds budget".into());
        }
        let item = item.map_err(|e| e.to_string())?;
        let net = item.network().map_err(|e| e.to_string())?;
        let ip = net.ip();
        let flat = item
            .decode::<Flat>()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let network = ipnet::IpNet::new(ip, net.prefix()).map_err(|e| e.to_string())?;
        let family = usize::from(ip.is_ipv6());
        if let Some(code) = flat.country_code {
            if net.prefix() == 0 || !super::mmdb::country_code(code) {
                return Err("Invalid ipgeo country record".into());
            }
            let record = geoip2::Country {
                country: geoip2::country::Country {
                    iso_code: Some(code),
                    ..Default::default()
                },
                ..Default::default()
            };
            country
                .insert(network, &record)
                .map_err(|e| e.to_string())?;
            seen[0][family] += 1;
        }
        if let Some(number) = flat.asn {
            if number == 0 || net.prefix() == 0 {
                return Err("Invalid ipgeo ASN record".into());
            }
            let record = geoip2::Asn {
                autonomous_system_number: Some(number),
                autonomous_system_organization: flat.as_org.filter(|s| !s.is_empty()),
            };
            asn.insert(network, &record).map_err(|e| e.to_string())?;
            seen[1][family] += 1;
        }
        if index.is_multiple_of(4096) {
            samples.push((ip, flat));
        }
    }
    if seen.iter().flatten().any(|count| *count == 0) {
        return Err("ipgeo is missing Country or ASN coverage for an address family".into());
    }
    let country_bytes = country.to_bytes().map_err(|e| e.to_string())?;
    drop(country);
    let output = Databases {
        country: country_bytes,
        asn: asn.to_bytes().map_err(|e| e.to_string())?,
    };
    drop(asn);
    let countries = Reader::from_source(&output.country).map_err(|e| e.to_string())?;
    let asns = Reader::from_source(&output.asn).map_err(|e| e.to_string())?;
    for (ip, original) in samples {
        let code = countries
            .lookup(ip)
            .and_then(|r| r.decode::<geoip2::Country>())
            .map_err(|e| e.to_string())?
            .and_then(|r| r.country.iso_code);
        let record = asns
            .lookup(ip)
            .and_then(|r| r.decode::<geoip2::Asn>())
            .map_err(|e| e.to_string())?;
        if code != original.country_code
            || record.as_ref().and_then(|r| r.autonomous_system_number) != original.asn
            || record.and_then(|r| r.autonomous_system_organization)
                != original.asn.and(original.as_org).filter(|s| !s.is_empty())
        {
            return Err(format!("ipgeo conversion changed lookup at {ip}"));
        }
    }
    println!("[INFO] ipgeo converted Country and ASN networks (v4/v6): {seen:?}");
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipnet::IpNet;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Entry<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        country_code: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        asn: Option<u32>,
        as_org: &'a str,
    }

    #[test]
    fn typed_outputs_preserve_each_field_independently_and_leave_gaps_empty() {
        let mut writer = Writer::builder("ipgeo")
            .ipv4_aliasing(Ipv4Aliasing::Disabled)
            .build();
        for (cidr, code, asn) in [
            ("192.0.2.0/25", Some("JP"), None),
            ("192.0.2.128/25", None, Some(64512)),
            ("2001:db8::/48", Some("TW"), Some(64513)),
        ] {
            writer
                .insert(
                    cidr.parse::<IpNet>().unwrap(),
                    &Entry {
                        country_code: code,
                        asn,
                        as_org: "Example",
                    },
                )
                .unwrap();
        }
        let bytes = writer.to_bytes().unwrap();
        let output = convert(&bytes).unwrap();
        let country = Reader::from_source(output.country).unwrap();
        let asn = Reader::from_source(output.asn).unwrap();
        assert_eq!(country.metadata().database_type, "GeoLite2-Country");
        assert_eq!(asn.metadata().database_type, "GeoLite2-ASN");
        for (address, code, number) in [
            ("192.0.2.127", Some("JP"), None),
            ("192.0.2.128", None, Some(64512)),
            ("2001:db8::1", Some("TW"), Some(64513)),
            ("2001:db8:1::", None, None),
            ("198.51.100.1", None, None),
        ] {
            let ip = address.parse().unwrap();
            assert_eq!(
                country
                    .lookup(ip)
                    .unwrap()
                    .decode::<geoip2::Country>()
                    .unwrap()
                    .and_then(|r| r.country.iso_code),
                code
            );
            let record = asn.lookup(ip).unwrap().decode::<geoip2::Asn>().unwrap();
            assert_eq!(
                record.as_ref().and_then(|r| r.autonomous_system_number),
                number
            );
            assert_eq!(
                record.and_then(|r| r.autonomous_system_organization),
                number.map(|_| "Example")
            );
        }
        assert!(convert(b"not a database").is_err());
        assert!(convert(&Writer::new("GeoLite2-Country").to_bytes().unwrap()).is_err());
    }
}
