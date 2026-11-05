use std::collections::BTreeSet;

const IPV4_FIXED: &[&str] = &[
    "0.in-addr.arpa",
    "10.in-addr.arpa",
    "127.in-addr.arpa",
    "168.192.in-addr.arpa",
    "254.169.in-addr.arpa",
    "2.0.192.in-addr.arpa",
    "100.51.198.in-addr.arpa",
    "113.0.203.in-addr.arpa",
    "255.255.255.255.in-addr.arpa",
];
const IPV6_FIXED: &[&str] = &[
    "0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.ip6.arpa",
    "1.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.ip6.arpa",
    "d.f.ip6.arpa",
    "8.e.f.ip6.arpa",
    "9.e.f.ip6.arpa",
    "a.e.f.ip6.arpa",
    "b.e.f.ip6.arpa",
    "8.b.d.0.1.0.0.2.ip6.arpa",
];

fn numbered(name: &str, suffix: &str, range: std::ops::RangeInclusive<u8>) -> bool {
    name.strip_suffix(suffix)
        .and_then(|n| n.parse::<u8>().ok().filter(|v| n == v.to_string()))
        .is_some_and(|n| range.contains(&n))
}

pub(super) fn local_zone(name: &str) -> bool {
    IPV4_FIXED.contains(&name)
        || IPV6_FIXED.contains(&name)
        || numbered(name, ".172.in-addr.arpa", 16..=31)
        || numbered(name, ".100.in-addr.arpa", 64..=127)
}

pub(super) fn wildcard_zone(name: &str) -> bool {
    let full_address = if name.ends_with(".ip6.arpa") {
        name.split('.').count() == 34
    } else {
        name.split('.').count() == 6
    };
    local_zone(name) && !full_address
}

fn baseline(ipv6: bool) -> BTreeSet<String> {
    let mut zones: BTreeSet<String> = if ipv6 { IPV6_FIXED } else { IPV4_FIXED }
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    if !ipv6 {
        zones.extend((16..=31).map(|n| format!("{n}.172.in-addr.arpa")));
        zones.extend((64..=127).map(|n| format!("{n}.100.in-addr.arpa")));
    }
    zones
}

// shortcut: one-line CSV records only; reject upstream multiline fields until reviewed.
fn fields(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        let mut field = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') if chars.peek() == Some(&'"') => {
                        chars.next();
                        field.push('"');
                    }
                    Some('"') => break,
                    Some(c) => field.push(c),
                    None => return Err("unclosed IANA CSV quote".into()),
                }
            }
            if chars.peek().is_some_and(|c| *c != ',') {
                return Err("invalid IANA CSV after quote".into());
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c == ',' {
                    break;
                }
                if c == '"' {
                    return Err("invalid IANA CSV quote".into());
                }
                field.push(c);
                chars.next();
            }
        }
        fields.push(field);
        if chars.next().is_none() {
            break;
        }
    }
    Ok(fields)
}

fn valid_zone(name: &str, ipv6: bool) -> bool {
    if let Some(prefix) = name.strip_suffix(if ipv6 { ".ip6.arpa" } else { ".in-addr.arpa" }) {
        let labels: Vec<_> = prefix.split('.').collect();
        if ipv6 {
            !labels.is_empty()
                && labels.len() <= 32
                && labels
                    .iter()
                    .all(|n| n.len() == 1 && n.bytes().all(|c| c.is_ascii_hexdigit()))
        } else {
            !labels.is_empty()
                && labels.len() <= 4
                && labels
                    .iter()
                    .all(|n| n.parse::<u8>().ok().is_some_and(|v| *n == v.to_string()))
        }
    } else {
        false
    }
}

pub(super) fn filtered(source: &str, ipv6: bool) -> Result<BTreeSet<String>, String> {
    let mut lines = source.lines();
    if lines.next().map(|s| s.trim_end_matches('\r')) != Some("Zone,Description,Reference") {
        return Err("invalid IANA CSV header".into());
    }
    let mut seen = BTreeSet::new();
    for line in lines {
        let row = fields(line.trim_end_matches('\r'))?;
        if row.len() != 3 || row.iter().any(|field| field.trim().is_empty()) {
            return Err("invalid IANA CSV row".into());
        }
        let zone = row[0].to_ascii_lowercase();
        if !valid_zone(&zone, ipv6) || !seen.insert(zone) {
            return Err("invalid or duplicate IANA zone".into());
        }
    }
    let required = baseline(ipv6);
    // IANA registrations are permanent; missing reviewed zones indicate an incomplete source.
    if !required.is_subset(&seen) {
        return Err("IANA CSV is missing reviewed zones".into());
    }
    Ok(host_names(seen))
}

fn host_names(zones: BTreeSet<String>) -> BTreeSet<String> {
    let mut hosts = BTreeSet::new();
    for zone in zones.into_iter().filter(|zone| local_zone(zone)) {
        if wildcard_zone(&zone) {
            hosts.insert(format!("*.{zone}"));
        }
        hosts.insert(zone);
    }
    hosts
}

pub(super) fn expected_hosts(ipv6: bool) -> BTreeSet<String> {
    host_names(baseline(ipv6))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv(ipv6: bool) -> String {
        let mut text = String::from("Zone,Description,Reference\n");
        for zone in baseline(ipv6) {
            text.push_str(&format!("{zone},\"a, \"\"quoted\"\" description\",[RFC]\n"));
        }
        text
    }

    #[test]
    fn validates_complete_registry_and_csv() {
        for ipv6 in [false, true] {
            let source = csv(ipv6);
            assert!(filtered(&source, ipv6).is_ok());
            assert!(
                filtered(
                    &source.replacen("Zone,Description,Reference", "bad", 1),
                    ipv6
                )
                .is_err()
            );
            let lines: Vec<_> = source.lines().collect();
            assert!(filtered(&lines[..lines.len() - 1].join("\n"), ipv6).is_err());
            let middle = lines.len() / 2;
            let missing = lines
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != middle)
                .map(|(_, line)| *line)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(filtered(&missing, ipv6).is_err());
            assert!(filtered(&(source.clone() + lines[1] + "\n"), ipv6).is_err());
            assert!(filtered(&(source + "bad,\"unterminated,x\n"), ipv6).is_err());
        }
    }

    #[test]
    fn malformed_or_multiline_csv_does_not_pass_source_validation() {
        for row in [
            "1.2.in-addr.arpa,\"unclosed,[RFC]",
            "1.2.in-addr.arpa,\"closed\"trailing,[RFC]",
            "1.2.in-addr.arpa,invalid\"quote,[RFC]",
            "1.2.in-addr.arpa,\"line one\nline two\",[RFC]",
            "1.2.in-addr.arpa,description,[RFC],extra",
            "1.2.in-addr.arpa,description,",
        ] {
            assert!(
                filtered(&(csv(false) + row + "\n"), false).is_err(),
                "{row}"
            );
        }
    }

    #[test]
    fn admits_only_reviewed_canonical_zones() {
        for zone in [
            "64.100.in-addr.arpa",
            "127.100.in-addr.arpa",
            "16.172.in-addr.arpa",
            "31.172.in-addr.arpa",
        ] {
            assert!(local_zone(zone));
        }
        for zone in [
            "63.100.in-addr.arpa",
            "128.100.in-addr.arpa",
            "064.100.in-addr.arpa",
            "8.8.8.in-addr.arpa",
        ] {
            assert!(!local_zone(zone));
        }
        let hosts = filtered(&(csv(false) + "8.8.8.in-addr.arpa,public,[test]\n"), false).unwrap();
        assert!(!hosts.contains("8.8.8.in-addr.arpa"));
        assert!(!hosts.contains("*.255.255.255.255.in-addr.arpa"));
        let hosts6 = filtered(&csv(true), true).unwrap();
        for zone in &IPV6_FIXED[..2] {
            assert!(hosts6.contains(*zone));
            assert!(!hosts6.contains(&format!("*.{zone}")));
        }
        assert!(!valid_zone(
            &format!(
                "0.{}.ip6.arpa",
                IPV6_FIXED[0].strip_suffix(".ip6.arpa").unwrap()
            ),
            true
        ));
    }
}
