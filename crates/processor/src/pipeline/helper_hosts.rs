use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::promax::source::{DownloadConfig, Downloader};

mod iana;

const SNAPSHOT: &str = "modules/source/local/sukka-lan-hosts.txt";
const MAX_SOURCE_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy)]
enum Format {
    Sukka,
    Iana { ipv6: bool },
}

#[derive(Clone, Copy)]
struct Feed {
    url: &'static str,
    snapshot: &'static str,
    attribution: &'static str,
    format: Format,
}

const FEEDS: &[Feed] = &[
    Feed {
        url: "https://ruleset.skk.moe/Modules/Rules/sukka_local_dns_mapping/lan_without_real_ip.conf",
        snapshot: SNAPSHOT,
        attribution: "SukkaW/Surge (source data: AGPL-3.0)",
        format: Format::Sukka,
    },
    Feed {
        url: "https://www.iana.org/assignments/locally-served-dns-zones/ipv4.csv",
        snapshot: "modules/source/local/iana-ipv4-local-zones.txt",
        attribution: "IANA IPv4 Locally-Served DNS Zones (registry data: CC0-1.0)",
        format: Format::Iana { ipv6: false },
    },
    Feed {
        url: "https://www.iana.org/assignments/locally-served-dns-zones/ipv6.csv",
        snapshot: "modules/source/local/iana-ipv6-local-zones.txt",
        attribution: "IANA IPv6 Locally-Served DNS Zones (registry data: CC0-1.0)",
        format: Format::Iana { ipv6: true },
    },
];

pub(super) fn source_comments() -> impl Iterator<Item = String> {
    FEEDS.iter().map(|feed| {
        format!(
            "# Local names selected from {}: {}",
            feed.attribution, feed.url
        )
    })
}

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
        "home" | "home.arpa" | "internal" | "lan" | "local" | "localdomain" | "localhost"
    ) || iana::wildcard_zone(name)
}

fn supported(name: &str) -> bool {
    if let Some(zone) = name.strip_prefix("*.") {
        local_zone(zone)
    } else {
        local_zone(name) || iana::local_zone(name) || DEVICE_NAMES.contains(&name)
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

fn publish_feed(root: &Path, feed: Feed, source: &str) -> Result<(), String> {
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err("Host source exceeds its byte budget".into());
    }
    let hosts = match feed.format {
        Format::Sukka => filtered(source)?,
        Format::Iana { ipv6 } => iana::filtered(source, ipv6)?,
    };
    let snapshot = root.join(feed.snapshot);
    match fs::symlink_metadata(&snapshot) {
        Ok(metadata) if !metadata.is_file() => {
            return Err("Host snapshot must be a regular file".into());
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("cannot inspect Host snapshot: {error}"));
        }
        _ => {}
    }
    let text = format!(
        "# Filtered local Host names from {}\n# {}\n{}\n",
        feed.attribution,
        feed.url,
        hosts.into_iter().collect::<Vec<_>>().join("\n")
    );
    if !crate::safe_write_file_internal(&snapshot, &text, true) {
        return Err("could not publish filtered local Host names".into());
    }
    Ok(())
}

fn refresh_with(
    root: &Path,
    mut fetch: impl FnMut(&str) -> Result<String, String>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    for feed in FEEDS {
        if let Err(error) = fetch(feed.url).and_then(|source| publish_feed(root, *feed, &source)) {
            failures.push(format!("{}: {error}", feed.url));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Helper DNS refresh failed for these sources (no replacement published for them): {}",
            failures.join("; ")
        ))
    }
}

pub(super) fn refresh(root: &Path) -> Result<(), String> {
    let downloader = Downloader::new(DownloadConfig {
        attempts: 2,
        timeout: Duration::from_secs(12),
        batch_timeout: Duration::from_secs(30),
        concurrency: 1,
        max_bytes: MAX_SOURCE_BYTES,
        max_total_bytes: MAX_SOURCE_BYTES * FEEDS.len() as u64,
        ..DownloadConfig::default()
    });
    refresh_with(root, |url| {
        downloader.get(url).map_err(|error| error.to_string())
    })
}

fn load_feed(root: &Path, feed: Feed) -> Result<BTreeSet<String>, String> {
    let snapshot = root.join(feed.snapshot);
    let metadata = fs::symlink_metadata(&snapshot)
        .map_err(|error| format!("cannot inspect {}: {error}", feed.snapshot))?;
    if !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES {
        return Err(format!("{} must be a bounded regular file", feed.snapshot));
    }
    let text = fs::read_to_string(snapshot)
        .map_err(|error| format!("cannot read {}: {error}", feed.snapshot))?;
    let hosts = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    let hosts: BTreeSet<_> = hosts
        .map(|name| {
            let correct_source = match feed.format {
                Format::Sukka => true,
                Format::Iana { ipv6 } => name
                    .strip_prefix("*.")
                    .unwrap_or(name)
                    .ends_with(if ipv6 { ".ip6.arpa" } else { ".in-addr.arpa" }),
            };
            (supported(name) && correct_source)
                .then(|| name.to_string())
                .ok_or_else(|| format!("unsafe Host in {}", feed.snapshot))
        })
        .collect::<Result<_, _>>()?;
    if hosts.is_empty() {
        return Err(format!("{} is empty", feed.snapshot));
    }
    if let Format::Iana { ipv6 } = feed.format
        && !iana::expected_hosts(ipv6).is_subset(&hosts)
    {
        return Err(format!("{} is missing reviewed local zones", feed.snapshot));
    }
    Ok(hosts)
}

pub(super) fn load(root: &Path) -> Result<Vec<String>, String> {
    let mut hosts = BTreeSet::new();
    for feed in FEEDS {
        hosts.extend(load_feed(root, *feed)?);
    }
    Ok(hosts.into_iter().collect())
}

#[cfg(test)]
pub(super) fn seed_test_snapshots(root: &Path) {
    fs::create_dir_all(root.join("modules/source/local")).unwrap();
    refresh_with(root, |url| {
        if url == FEEDS[0].url {
            Ok("# Size: 1\nDOMAIN,unifi\n### EOF ###".into())
        } else {
            let ipv6 = url == FEEDS[2].url;
            let mut source = String::from("Zone,Description,Reference\n");
            for host in iana::expected_hosts(ipv6)
                .iter()
                .filter(|host| !host.starts_with("*."))
            {
                source.push_str(&format!("{host},Local zone,[RFC]\n"));
            }
            Ok(source)
        }
    })
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn publish(root: &Path, source: &str) -> Result<(), String> {
        publish_feed(root, FEEDS[0], source)
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("script-hub-{label}-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn source_failures_do_not_block_other_updates_or_replace_their_last_good_snapshots() {
        let root = temp_root("helper-feed-recovery");
        seed_test_snapshots(&root);
        let original: Vec<_> = FEEDS
            .iter()
            .map(|feed| fs::read(root.join(feed.snapshot)).unwrap())
            .collect();
        let error = refresh_with(&root, |url| {
            if url == FEEDS[0].url {
                Ok("# Size: 2\nDOMAIN,pi.hole\nDOMAIN,unifi\n### EOF ###".into())
            } else if url == FEEDS[1].url {
                Err("network unavailable".into())
            } else {
                Ok("Zone,Description,Reference\nd.f.ip6.arpa,Truncated,[RFC]\n".into())
            }
        })
        .unwrap_err();
        assert!(error.contains(FEEDS[1].url) && error.contains(FEEDS[2].url));
        assert!(load(&root).unwrap().iter().any(|host| host == "pi.hole"));
        for (feed, expected) in FEEDS[1..].iter().zip(&original[1..]) {
            assert_eq!(&fs::read(root.join(feed.snapshot)).unwrap(), expected);
        }
        let updated: Vec<_> = FEEDS
            .iter()
            .map(|feed| fs::read(root.join(feed.snapshot)).unwrap())
            .collect();
        assert!(refresh_with(&root, |_| Err("offline".into())).is_err());
        for (feed, expected) in FEEDS.iter().zip(updated) {
            assert_eq!(fs::read(root.join(feed.snapshot)).unwrap(), expected);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_incomplete_or_unsafe_source_snapshots_block_generation() {
        let root = temp_root("helper-snapshot-validation");
        seed_test_snapshots(&root);
        let path = root.join(FEEDS[2].snapshot);
        let original = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(load(&root).is_err());
        for corrupted in [
            "d.f.ip6.arpa\n".to_string(),
            format!("{original}*.{}\n", "0.".repeat(32) + "ip6.arpa"),
            format!("{original}*.apple.com\n"),
            format!("{original}64.100.in-addr.arpa\n"),
        ] {
            fs::write(&path, corrupted).unwrap();
            assert!(load(&root).is_err());
        }
        fs::write(&path, original).unwrap();
        assert!(load(&root).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repeated_refresh_preserves_snapshot_bytes_and_modification_times() {
        let root = temp_root("helper-feed-noop");
        seed_test_snapshots(&root);
        let original: Vec<_> = FEEDS
            .iter()
            .map(|feed| {
                let path = root.join(feed.snapshot);
                (
                    fs::read(&path).unwrap(),
                    fs::metadata(path).unwrap().modified().unwrap(),
                )
            })
            .collect();
        seed_test_snapshots(&root);
        for (feed, expected) in FEEDS.iter().zip(original) {
            let path = root.join(feed.snapshot);
            assert_eq!(
                (
                    fs::read(&path).unwrap(),
                    fs::metadata(path).unwrap().modified().unwrap()
                ),
                expected
            );
        }
        let hosts = load(&root).unwrap();
        assert!(hosts.windows(2).all(|pair| pair[0] < pair[1]));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn source_refresh_does_not_follow_or_replace_snapshot_symlinks() {
        let root = temp_root("helper-snapshot-symlink");
        seed_test_snapshots(&root);
        let target = root.join("sentinel.txt");
        fs::write(&target, "unchanged\n").unwrap();
        let path = root.join(FEEDS[0].snapshot);
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(publish(&root, "# Size: 1\nDOMAIN,unifi\n### EOF ###").is_err());
        assert!(load(&root).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "unchanged\n");
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
        fs::remove_dir_all(root).unwrap();
    }

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
        assert_eq!(
            load_feed(&root, FEEDS[0])
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            ["pi.hole"]
        );
        fs::write(path, "*.apple.com\n").unwrap();
        assert!(load_feed(&root, FEEDS[0]).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "bounded live HTTPS requests to the official Sukka and IANA feeds"]
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
        let snapshot_unchanged = FEEDS.iter().all(|feed| {
            fs::read(root.join(feed.snapshot)).unwrap()
                == fs::read(repository.join(feed.snapshot)).unwrap()
        });
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
