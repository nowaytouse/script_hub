//! Country-only projection of the source MMDB's exact network membership.
//!
//! MaxMind's iterator reports the canonical IPv4 subtree as IPv4 and skips its
//! IPv6 aliases by default. Coordinates and registered/nearest-country fields
//! are deliberately not used to infer a country.

use maxminddb::{PathElement, Reader, WithinOptions};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub(super) const MAX_DB_BYTES: usize = 192 * 1024 * 1024;
const MAX_NETWORKS: usize = 32_000_000;
const MAX_COUNTRIES: usize = 256;
const COUNTRY_PATH: [PathElement<'static>; 2] =
    [PathElement::Key("country"), PathElement::Key("iso_code")];

#[derive(Debug)]
pub(super) struct Extraction {
    pub(super) database_type: String,
    pub(super) build_epoch: u64,
    pub(super) ip_version: u16,
    /// Canonical CIDRs, with IPv4 before IPv6, aggregated within each country.
    pub(super) countries: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Network {
    family: u8,
    address: u128,
    prefix: u8,
}

impl Network {
    fn new(address: IpAddr, prefix: u8) -> Result<Self, String> {
        let (family, number) = match address {
            IpAddr::V4(address) => (4, u128::from(u32::from(address))),
            IpAddr::V6(address) => (6, u128::from(address)),
        };
        let width = if family == 4 { 32 } else { 128 };
        if prefix == 0 || prefix > width {
            return Err("MMDB country network has an invalid or default prefix".into());
        }
        Ok(Self {
            family,
            address: number & (u128::MAX << (width - prefix)),
            prefix,
        })
    }

    fn width(self) -> u8 {
        if self.family == 4 { 32 } else { 128 }
    }

    fn covers(self, other: Self) -> bool {
        self.family == other.family
            && self.prefix <= other.prefix
            && other.address & (u128::MAX << (self.width() - self.prefix)) == self.address
    }

    fn sibling_parent(self, other: Self) -> Option<Self> {
        if self.family != other.family || self.prefix != other.prefix {
            return None;
        }
        let block = 1_u128 << (self.width() - self.prefix);
        if self.address ^ other.address != block {
            return None;
        }
        Some(Self {
            family: self.family,
            address: self.address.min(other.address),
            prefix: self.prefix - 1,
        })
    }

    fn cidr(self) -> String {
        let address = if self.family == 4 {
            IpAddr::V4(Ipv4Addr::from(self.address as u32))
        } else {
            IpAddr::V6(Ipv6Addr::from(self.address))
        };
        format!("{address}/{}", self.prefix)
    }
}

fn country_code(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_uppercase())
}

/// Deduplication, containment removal and sibling aggregation preserve the
/// union exactly. In particular, distinct countries never share this stack.
fn collapse(mut networks: Vec<Network>) -> Result<Vec<String>, String> {
    networks.sort_unstable();
    let mut output: Vec<Network> = Vec::with_capacity(networks.len());
    for mut network in networks {
        loop {
            let Some(previous) = output.last().copied() else {
                output.push(network);
                break;
            };
            if previous.covers(network) {
                break;
            }
            if network.covers(previous) {
                output.pop();
                continue;
            }
            if let Some(parent) = previous.sibling_parent(network) {
                if parent.prefix == 0 {
                    return Err("MMDB country aggregation would produce a default route".into());
                }
                output.pop();
                network = parent;
                continue;
            }
            output.push(network);
            break;
        }
    }
    Ok(output.into_iter().map(Network::cidr).collect())
}

pub(super) fn extract(bytes: &[u8], countries: &[String]) -> Result<Extraction, String> {
    if bytes.is_empty() || bytes.len() > MAX_DB_BYTES {
        return Err("MMDB input is empty or exceeds the extraction size limit".into());
    }
    if countries.is_empty() || countries.len() > MAX_COUNTRIES {
        return Err("MMDB extraction requires a bounded nonempty country selection".into());
    }
    let mut selected: BTreeMap<String, Vec<Network>> = BTreeMap::new();
    for country in countries {
        if !country_code(country) || selected.insert(country.clone(), Vec::new()).is_some() {
            return Err("MMDB country selection contains an invalid or duplicate code".into());
        }
    }
    let reader = Reader::from_source(bytes).map_err(|_| "Invalid MMDB database".to_owned())?;
    let metadata = reader.metadata();
    if !matches!(metadata.ip_version, 4 | 6)
        || metadata.database_type.is_empty()
        || metadata.database_type.len() > 128
        || metadata.database_type.chars().any(char::is_control)
        || metadata.node_count as usize > MAX_NETWORKS
    {
        return Err("MMDB metadata exceeds the supported extraction limits".into());
    }
    // Including no-data leaves makes them count against the traversal budget.
    // Do not opt into aliased IPv4 networks or skip malformed empty records.
    let options = WithinOptions::default().include_networks_without_data();
    let networks = reader
        .networks(options)
        .map_err(|_| "Invalid MMDB search tree".to_owned())?;
    for (index, item) in networks.enumerate() {
        if index >= MAX_NETWORKS {
            return Err("MMDB traversal exceeds the extraction network limit".into());
        }
        let lookup = item.map_err(|_| "Invalid MMDB network record".to_owned())?;
        if !lookup.has_data() {
            continue;
        }
        let country: &str = lookup
            .decode_path(&COUNTRY_PATH)
            .map_err(|_| "MMDB country record has an invalid schema".to_owned())?
            .ok_or_else(|| "MMDB country record is missing country.iso_code".to_owned())?;
        if !country_code(country) {
            return Err("MMDB country record has an invalid country code".into());
        }
        let network = lookup
            .network()
            .map_err(|_| "Invalid MMDB country network".to_owned())?;
        let network = Network::new(network.ip(), network.prefix())?;
        if let Some(output) = selected.get_mut(country) {
            output.push(network);
        }
    }
    let mut output = BTreeMap::new();
    for (country, networks) in selected {
        if networks.is_empty() {
            return Err("Requested MMDB country contains no networks".into());
        }
        let cidrs = collapse(networks)?;
        output.insert(country, cidrs);
    }
    Ok(Extraction {
        database_type: metadata.database_type.clone(),
        build_epoch: metadata.build_epoch,
        ip_version: metadata.ip_version,
        countries: output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(value: &str) -> Network {
        let (address, prefix) = value.split_once('/').unwrap();
        Network::new(address.parse().unwrap(), prefix.parse().unwrap()).unwrap()
    }

    // Minimal synthetic 24-bit-node MMDB. The two data records cover the two
    // halves of IPv4 space. No real database or source dump is used as a fixture.
    fn database(left_country_field: &str, left: &str, right: &str) -> Vec<u8> {
        fn string(output: &mut Vec<u8>, value: &str) {
            assert!(value.len() < 29);
            output.push(0x40 | value.len() as u8);
            output.extend_from_slice(value.as_bytes());
        }
        fn record(field: &str, country: &str) -> Vec<u8> {
            let mut output = vec![0xe1];
            string(&mut output, field);
            output.push(0xe1);
            string(&mut output, "iso_code");
            string(&mut output, country);
            output
        }
        let first = record(left_country_field, left);
        let second = record("country", right);
        let mut output = Vec::new();
        for pointer in [17_u32, 17 + first.len() as u32] {
            output.extend_from_slice(&pointer.to_be_bytes()[1..]);
        }
        output.extend_from_slice(&[0; 16]);
        output.extend(first);
        output.extend(second);
        output.extend_from_slice(b"\xab\xcd\xefMaxMind.com");
        output.push(0xe9);
        for (name, value) in [
            ("binary_format_major_version", 2),
            ("binary_format_minor_version", 0),
            ("ip_version", 4),
            ("record_size", 24),
        ] {
            string(&mut output, name);
            output.extend_from_slice(&[0xa1, value]);
        }
        string(&mut output, "node_count");
        output.extend_from_slice(&[0xc1, 1]);
        string(&mut output, "build_epoch");
        output.extend_from_slice(&[1, 2, 1]);
        string(&mut output, "database_type");
        string(&mut output, "GeoIP2-City");
        string(&mut output, "languages");
        output.extend_from_slice(&[1, 4]);
        string(&mut output, "en");
        string(&mut output, "description");
        output.push(0xe1);
        string(&mut output, "en");
        string(&mut output, "Synthetic fixture");
        output
    }

    #[test]
    fn aggregation_preserves_both_families_and_removes_only_covered_ranges() {
        let values = [
            "192.0.2.0/25",
            "192.0.2.128/25",
            "192.0.2.7/32",
            "192.0.2.0/25",
            "192.0.3.0/24",
            "2001:db8::/33",
            "2001:db8:8000::/33",
        ];
        let actual = collapse(values.into_iter().map(network).collect()).unwrap();
        assert_eq!(actual, ["192.0.2.0/23", "2001:db8::/32"]);
        assert_eq!(
            Network::new("192.0.2.177".parse().unwrap(), 24)
                .unwrap()
                .cidr(),
            "192.0.2.0/24"
        );
        assert_eq!(
            Network::new("2001:db8::1234".parse().unwrap(), 64)
                .unwrap()
                .cidr(),
            "2001:db8::/64"
        );
    }

    #[test]
    fn aggregation_never_fills_a_gap_or_generates_a_default_route() {
        assert_eq!(
            collapse(vec![network("192.0.2.0/26"), network("192.0.2.128/26")]).unwrap(),
            ["192.0.2.0/26", "192.0.2.128/26"]
        );
        assert!(collapse(vec![network("0.0.0.0/1"), network("128.0.0.0/1")]).is_err());
        assert!(collapse(vec![network("::/1"), network("8000::/1")]).is_err());
        assert!(Network::new("0.0.0.0".parse().unwrap(), 0).is_err());
    }

    #[test]
    fn country_projection_uses_only_exact_country_and_retains_separate_memberships() {
        let result = extract(
            &database("country", "CN", "US"),
            &["CN".into(), "US".into()],
        )
        .unwrap();
        assert_eq!(result.ip_version, 4);
        assert_eq!(result.database_type, "GeoIP2-City");
        assert_eq!(result.build_epoch, 1);
        assert_eq!(result.countries["CN"], ["0.0.0.0/1"]);
        assert_eq!(result.countries["US"], ["128.0.0.0/1"]);
        assert!(extract(&database("registered_country", "CN", "US"), &["CN".into()]).is_err());
        assert!(extract(&database("country", "cn", "US"), &["US".into()]).is_err());
        assert!(extract(&database("country", "CN", "US"), &["JP".into()]).is_err());
    }

    #[test]
    fn invalid_data_and_country_selections_fail_without_echoing_payloads() {
        for bytes in [
            b"".as_slice(),
            b"fixture-private-payload".as_slice(),
            &[0; 64],
        ] {
            let error = extract(bytes, &["CN".into()]).unwrap_err();
            assert!(!error.contains("fixture-private-payload"));
        }
        let valid = database("country", "CN", "US");
        for selected in [
            vec![],
            vec!["cn".into()],
            vec!["C1".into()],
            vec!["CN".into(), "CN".into()],
        ] {
            assert!(extract(&valid, &selected).is_err());
        }
    }
}
