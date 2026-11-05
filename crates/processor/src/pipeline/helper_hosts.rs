use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::promax::source::{DownloadConfig, Downloader};

pub(super) const SOURCE: &str =
    "https://ruleset.skk.moe/Modules/Rules/sukka_local_dns_mapping/lan_without_real_ip.conf";
const SNAPSHOT: &str = "modules/source/local/sukka-lan-hosts.txt";

// Public-looking device aliases may be admitted only as exact names.
const DEVICE_NAMES: &[&str] = &[
    "asusrouter.com",
    "aterm.me",
    "console.gl-inet.com",
    "homerouter.cpe",
    "huaweimobilewifi.com",
    "instant.arubanetworks.com",
    "mobile.hotspot",
    "ntt.setup",
    "pi.hole",
    "repeater.asus.com",
    "router.asus.com",
    "routerlogin.com",
    "routerlogin.net",
    "setmeup.arubanetworks.com",
    "tplinkap.net",
    "tplinkmodem.net",
    "tplinkplclogin.net",
    "tplinkrepeater.net",
    "tplinkwifi.net",
    "tplogin.cn",
    "unifi",
];

pub(super) fn local_zone(name: &str) -> bool {
    matches!(
        name,
        "home"
            | "home.arpa"
            | "internal"
            | "lan"
            | "local"
            | "localdomain"
            | "localhost"
            | "10.in-addr.arpa"
            | "127.in-addr.arpa"
            | "168.192.in-addr.arpa"
            | "254.169.in-addr.arpa"
            | "d.f.ip6.arpa"
            | "8.e.f.ip6.arpa"
            | "9.e.f.ip6.arpa"
            | "a.e.f.ip6.arpa"
            | "b.e.f.ip6.arpa"
    ) || name
        .strip_suffix(".172.in-addr.arpa")
        .and_then(|n| {
            n.parse::<u8>()
                .ok()
                .filter(|number| n == number.to_string())
        })
        .is_some_and(|n| (16..=31).contains(&n))
}

fn supported(name: &str) -> bool {
    if let Some(zone) = name.strip_prefix("*.") {
        local_zone(zone)
    } else {
        local_zone(name) || DEVICE_NAMES.contains(&name)
    }
}

fn filtered(source: &str) -> Result<BTreeSet<String>, String> {
    if !source
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|line| line.starts_with('#') && line.trim_matches('#').trim() == "EOF")
    {
        return Err("Sukka LAN source is incomplete".into());
    }
    let mut declared_size = None;
    let mut row_count = 0;
    let mut hosts = BTreeSet::new();
    for line in source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(size) = line.strip_prefix("# Size:") {
            let size = size
                .trim()
                .parse::<usize>()
                .map_err(|_| "invalid Sukka LAN source size")?;
            if declared_size.replace(size).is_some() {
                return Err("duplicate Sukka LAN source size".into());
            }
        }
        if line.starts_with('#') {
            continue;
        }
        row_count += 1;
        let (kind, name) = line
            .split_once(',')
            .ok_or("malformed Sukka LAN source row")?;
        let name = name.trim().to_ascii_lowercase();
        if !matches!(kind, "DOMAIN" | "DOMAIN-SUFFIX") || name.contains(',') {
            return Err("unreviewed Sukka LAN source row format".into());
        }
        // Some Sukka feeds use an underscore-bearing provenance marker, never a Host entry.
        if name.ends_with(".skk.moe") && name.contains('_') {
            continue;
        }
        if name.len() > 253
            || name.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            })
        {
            return Err("invalid hostname in Sukka LAN source".into());
        }
        if !supported(&name) {
            continue;
        }
        if kind == "DOMAIN-SUFFIX" && local_zone(&name) {
            hosts.insert(format!("*.{name}"));
        }
        hosts.insert(name);
    }
    if declared_size != Some(row_count) {
        return Err("Sukka LAN source row count does not match its declared size".into());
    }
    if hosts.is_empty() {
        return Err("Sukka LAN source contains no supported hosts".into());
    }
    Ok(hosts)
}

pub(super) fn publish(root: &Path, source: &str) -> Result<(), String> {
    let hosts = filtered(source)?;
    let snapshot = root.join(SNAPSHOT);
    let text = format!(
        "# Filtered local Host names from SukkaW/Surge (source data: AGPL-3.0)\n# {SOURCE}\n{}\n",
        hosts.into_iter().collect::<Vec<_>>().join("\n")
    );
    if !crate::safe_write_file_internal(&snapshot, &text, true) {
        return Err("could not publish filtered Sukka LAN hosts".into());
    }
    Ok(())
}

pub(super) fn refresh(root: &Path) -> Result<(), String> {
    let downloader = Downloader::new(DownloadConfig {
        attempts: 2,
        timeout: Duration::from_secs(12),
        batch_timeout: Duration::from_secs(30),
        concurrency: 1,
        max_bytes: 64 * 1024,
        max_total_bytes: 64 * 1024,
        ..DownloadConfig::default()
    });
    let source = downloader.get(SOURCE).map_err(|error| error.to_string())?;
    publish(root, &source)
}

pub(super) fn load(root: &Path) -> Result<Vec<String>, String> {
    let snapshot = root.join(SNAPSHOT);
    let metadata = fs::symlink_metadata(&snapshot)
        .map_err(|error| format!("cannot inspect filtered Sukka LAN hosts: {error}"))?;
    if !metadata.is_file() {
        return Err("filtered Sukka LAN hosts must be a regular file".into());
    }
    let text = fs::read_to_string(snapshot)
        .map_err(|error| format!("cannot read filtered Sukka LAN hosts: {error}"))?;
    let hosts = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    let hosts: Vec<_> = hosts
        .map(|name| {
            supported(name)
                .then(|| name.to_string())
                .ok_or("unsafe Host in filtered Sukka snapshot")
        })
        .collect::<Result<_, _>>()?;
    if hosts.is_empty() {
        return Err("filtered Sukka LAN hosts are empty".into());
    }
    Ok(hosts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_cannot_insert_public_dns_or_broaden_device_aliases() {
        let source = "\
# Size: 12
DOMAIN-SUFFIX,ROUTERLOGIN.NET
DOMAIN-SUFFIX,168.192.in-addr.arpa
DOMAIN-SUFFIX,31.172.in-addr.arpa
DOMAIN-SUFFIX,home
DOMAIN,unifi
DOMAIN-SUFFIX,32.172.in-addr.arpa
DOMAIN-SUFFIX,8.8.8.in-addr.arpa
DOMAIN-SUFFIX,c.f.ip6.arpa
DOMAIN-SUFFIX,plex.direct
DOMAIN-SUFFIX,apple.com
DOMAIN,cdn.jsdelivr.net
DOMAIN,routerlogin.net.example.org
### EOF ###";
        let result = filtered(source).unwrap();
        assert_eq!(
            result.into_iter().collect::<Vec<_>>(),
            [
                "*.168.192.in-addr.arpa",
                "*.31.172.in-addr.arpa",
                "*.home",
                "168.192.in-addr.arpa",
                "31.172.in-addr.arpa",
                "home",
                "routerlogin.net",
                "unifi"
            ]
        );
    }

    #[test]
    fn provenance_marker_is_counted_but_never_published_as_a_host() {
        let hosts = filtered("# Size: 2\nDOMAIN,this_ruleset_is_made_by_sukkaw.ruleset.skk.moe\nDOMAIN,unifi\n### EOF ###").unwrap();
        assert_eq!(hosts.into_iter().collect::<Vec<_>>(), ["unifi"]);
    }

    #[test]
    fn incomplete_or_unsafe_only_refresh_does_not_replace_a_good_snapshot() {
        let root =
            std::env::temp_dir().join(format!("script-hub-local-hosts-{}", std::process::id()));
        fs::create_dir_all(root.join("modules/source/local")).unwrap();
        publish(&root, "# Size: 1\nDOMAIN,unifi\n### EOF ###").unwrap();
        let path = root.join(SNAPSHOT);
        let original = fs::read(&path).unwrap();
        for invalid in [
            "# Size: 1\nDOMAIN,pi.hole",
            "# Size: 2\nDOMAIN,pi.hole\n### EOF ###",
            "# Size: 1\nDOMAIN,pi.hole\n### not EOF ###",
            "# Size: 1\n# Size: 1\nDOMAIN,pi.hole\n### EOF ###",
            "DOMAIN,pi.hole\n### EOF ###",
            "# Size: 1\nDOMAIN,apple.com\n### EOF ###",
            "# Size: 1\nDOMAIN,unifi,REJECT\n### EOF ###",
            "# Size: 1\nIP-CIDR,192.168.0.0/16\n### EOF ###",
            "# Size: 1\nDOMAIN,*.lan\n### EOF ###",
            "# Size: 1\nDOMAIN,invalid..lan\n### EOF ###",
        ] {
            assert!(publish(&root, invalid).is_err());
            assert_eq!(fs::read(&path).unwrap(), original);
        }
        publish(&root, "# Size: 1\nDOMAIN,pi.hole\n### EOF ###").unwrap();
        assert_eq!(load(&root).unwrap(), ["pi.hole"]);
        fs::write(path, "*.apple.com\n").unwrap();
        assert!(load(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "bounded live HTTPS request to the official Sukka feed"]
    fn live_helper_refresh_runs_the_production_pipeline() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "script-hub-live-helper-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("modules/source/local")).unwrap();
        refresh(&root).unwrap();
        let hosts = load(&root).unwrap();
        assert!(!hosts.is_empty());
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let published_count = load(&repository).unwrap().len();
        let snapshot_unchanged =
            fs::read(root.join(SNAPSHOT)).unwrap() == fs::read(repository.join(SNAPSHOT)).unwrap();
        let version = "live-test";
        let written = crate::firewall_sync::refresh_security_modules(&root, version).unwrap();
        let source = repository.join("rulesets/Sources/conf/SurgeConf_DirectPorts.list");
        let targets = [
            root.join("modules/surge/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"),
            root.join("modules/shadowrocket/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.module"),
        ];
        let target_names: Vec<_> = targets.iter().map(|path| path.to_str().unwrap()).collect();
        crate::firewall_sync::sync_ports(source.to_str().unwrap(), &target_names, true, version);
        crate::url_rewriter::copy_github_variants(root.to_str().unwrap()).unwrap();
        crate::firewall_sync::validate_published_helpers(&root).unwrap();
        println!(
            "live Helper refresh: {} filtered Host names (published {published_count}), snapshot_unchanged={snapshot_unchanged}, {written} modules, DNS mirrors validated",
            hosts.len()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
