//! Client projections of the unlocked bundled Surge profile, not a general profile converter.

use super::{Client, Template, VaultError, inline_comment, links, slot_ranges};
use crate::pipeline::geo_rulesets::MMDB_URL;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

const RAW: &str = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/";
const CDN: &str = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/";
const ASN_URL: &str = "https://github.com/laincat/GeoIP/releases/latest/download/GeoLite2-ASN.mmdb";
const LOCAL_PROXY: &str = "script-hub-proxy";
const DOWNLOAD_CLIENT: &str = "ruleset-download";
const LAN: &[&str] = &[
    "DOMAIN-SUFFIX,local",
    "IP-CIDR,0.0.0.0/8,no-resolve",
    "IP-CIDR,10.0.0.0/8,no-resolve",
    "IP-CIDR,100.64.0.0/10,no-resolve",
    "IP-CIDR,127.0.0.0/8,no-resolve",
    "IP-CIDR,169.254.0.0/16,no-resolve",
    "IP-CIDR,172.16.0.0/12,no-resolve",
    "IP-CIDR,192.0.0.0/24,no-resolve",
    "IP-CIDR,192.0.2.0/24,no-resolve",
    "IP-CIDR,192.168.0.0/16,no-resolve",
    "IP-CIDR,224.0.0.0/4,no-resolve",
    "IP-CIDR6,::1/128,no-resolve",
    "IP-CIDR6,fc00::/7,no-resolve",
    "IP-CIDR6,fe80::/10,no-resolve",
];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum GroupKind {
    #[default]
    Select,
    UrlTest,
    Fallback,
    LoadBalance,
    Smart,
}

impl GroupKind {
    fn yaml_type(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::UrlTest | Self::Smart => "url-test",
            Self::Fallback => "fallback",
            Self::LoadBalance => "load-balance",
        }
    }
}

#[derive(Default)]
struct Group<'a> {
    kind: GroupKind,
    members: Vec<&'a str>,
    includes: Vec<&'a str>,
    slot: Option<&'a str>,
    filter: Option<&'a str>,
    interval: Option<u64>,
    tolerance: Option<u64>,
    test_timeout: Option<u64>,
    hidden: Option<bool>,
    persistent: Option<bool>,
    source_only: Vec<&'a str>,
}

fn boolean(value: &str) -> Result<bool, VaultError> {
    match value.trim_matches('"') {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(VaultError::Format),
    }
}

fn integer(value: &str) -> Result<u64, VaultError> {
    value
        .trim_matches('"')
        .parse()
        .map_err(|_| VaultError::Format)
}

fn endpoint(value: &str, https_only: bool) -> Result<&str, VaultError> {
    let parsed = url::Url::parse(value).map_err(|_| VaultError::Format)?;
    if !(parsed.scheme() == "https" || !https_only && parsed.scheme() == "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(VaultError::Format);
    }
    Ok(value)
}

// Surge quotes comma-bearing include-other-group, regex and modifier values.
fn fields(line: &str) -> impl Iterator<Item = &str> {
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    line.char_indices()
        .chain(std::iter::once((line.len(), ',')))
        .filter_map(move |(end, ch)| {
            if escaped {
                escaped = false;
            } else if quoted && ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = !quoted;
            } else if ch == ',' && !quoted {
                let field = line[start..end].trim();
                start = end + 1;
                return Some(field);
            }
            None
        })
}

fn group(line: &str) -> Result<(&str, Group<'_>), VaultError> {
    let (name, value) = line.split_once('=').ok_or(VaultError::Format)?;
    let name = name.trim();
    if name.is_empty()
        || name.contains([',', '"'])
        || name.chars().any(char::is_control)
        || ["DIRECT", "REJECT", "REJECT-DROP", LOCAL_PROXY, "direct"].contains(&name)
    {
        return Err(VaultError::Format);
    }
    let mut components = fields(value);
    let kind = match components.next().ok_or(VaultError::Format)? {
        "select" => GroupKind::Select,
        "url-test" => GroupKind::UrlTest,
        "fallback" => GroupKind::Fallback,
        "load-balance" => GroupKind::LoadBalance,
        "smart" => GroupKind::Smart,
        _ => return Err(VaultError::Format),
    };
    let mut group = Group {
        kind,
        ..Group::default()
    };
    let mut parameters = BTreeSet::new();
    for field in components {
        let Some((key, value)) = field.split_once('=') else {
            group.members.push(field.trim_matches('"'));
            continue;
        };
        let key = key.trim();
        if !parameters.insert(key) {
            return Err(VaultError::Format);
        }
        let value = value.trim().trim_matches('"');
        match key {
            "policy-path" => {
                let ranges = slot_ranges(value)?;
                if !matches!(ranges.as_slice(), [(0, end, _)] if *end == value.len()) {
                    return Err(VaultError::Format);
                }
                group.slot = Some(value);
            }
            "include-other-group" => group.includes.extend(value.split(',').map(str::trim)),
            "policy-regex-filter" => {
                regex::Regex::new(value).map_err(|_| VaultError::Format)?;
                group.filter = Some(value);
            }
            "interval" => group.interval = Some(integer(value)?),
            "tolerance" => group.tolerance = Some(integer(value)?),
            "test-timeout" => group.test_timeout = Some(integer(value)?),
            "hidden" => group.hidden = Some(boolean(value)?),
            "persistent" => group.persistent = Some(boolean(value)?),
            // Current Surge ignores group url=. Its timeout is a score threshold,
            // not Mihomo's health-check timeout. Do not silently substitute either.
            _ => group.source_only.push(key),
        }
    }
    Ok((name, group))
}

fn include_members<'a>(
    name: &'a str,
    groups: &BTreeMap<&'a str, Group<'a>>,
    visiting: &mut BTreeSet<&'a str>,
    providers: &mut Vec<&'a str>,
    members: &mut Vec<&'a str>,
) -> Result<(), VaultError> {
    if !visiting.insert(name) {
        return Err(VaultError::Format);
    }
    let group = groups.get(name).ok_or(VaultError::Format)?;
    for &member in &group.members {
        let member = yaml_policy(member, groups).ok_or(VaultError::Format)?;
        if !members.contains(&member) {
            members.push(member);
        }
    }
    // Keep each group's explicitly declared choices ahead of included choices.
    for &name in &group.includes {
        // A child filter constrains its nodes before inclusion. Combining those
        // filters is not modeled, so reject rather than broaden the selection.
        if groups.get(name).ok_or(VaultError::Format)?.filter.is_some() {
            return Err(VaultError::Format);
        }
        include_members(name, groups, visiting, providers, members)?;
    }
    if let Some(slot) = group.slot
        && !providers.contains(&slot)
    {
        providers.push(slot);
    }
    visiting.remove(name);
    Ok(())
}

fn derived_dns(general: &BTreeMap<&str, &str>) -> Result<Value, VaultError> {
    const BOOTSTRAP: &str = "https://223.5.5.5/dns-query#DIRECT";
    const DEFAULT: &str = "https://dns.alidns.com/dns-query";
    let nameservers = if let Some(source) = general.get("encrypted-dns-server") {
        fields(source)
            .map(|value| endpoint(value.trim_matches('"'), true).map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![DEFAULT.to_owned()]
    };
    if nameservers.is_empty() {
        return Err(VaultError::Format);
    }
    let ipv6 = general
        .get("ipv6")
        .map(|value| boolean(value))
        .transpose()?
        .unwrap_or(true);
    let follow_rules = general
        .get("encrypted-dns-follow-outbound-mode")
        .map(|value| boolean(value))
        .transpose()?
        .unwrap_or(false);
    let proxy_resolvers = nameservers
        .iter()
        .map(|value| format!("{value}#DIRECT"))
        .collect::<Vec<_>>();
    // Surge sends encrypted DNS directly unless the source explicitly opts into
    // outbound rules. Pin normal resolvers too, so an empty proxy group cannot
    // prevent the subscriptions needed to populate that group from resolving.
    let nameservers = nameservers
        .into_iter()
        .map(|value| {
            if follow_rules {
                value
            } else {
                format!("{value}#DIRECT")
            }
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "enable":true, "ipv6":ipv6,
        "enhanced-mode":"fake-ip", "fake-ip-range":"198.18.0.1/16",
        "use-hosts":true, "use-system-hosts":false, "respect-rules":true,
        "default-nameserver":[BOOTSTRAP],
        "proxy-server-nameserver":proxy_resolvers, "nameserver":nameservers,
        "fake-ip-filter":["+.lan", "+.local"],
    }))
}

fn inherit_geoip(general: &BTreeMap<&str, &str>, yaml: &mut Value) -> Result<(), VaultError> {
    let Some(source) = general.get("geoip-maxmind-url") else {
        return Ok(());
    };
    let source = endpoint(source.trim_matches('"'), true)?;
    // Surge also accepts archives, but Mihomo's MMDB downloader consumes the
    // response bytes directly rather than extracting a Country database.
    let path = url::Url::parse(source)
        .map_err(|_| VaultError::Format)?
        .path()
        .to_ascii_lowercase();
    if [".tar.gz", ".tgz", ".zip"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
    {
        return Err(VaultError::Format);
    }
    let disabled = general
        .get("disable-geoip-db-auto-update")
        .map(|value| boolean(value))
        .transpose()?
        .unwrap_or(false);
    yaml["geodata-mode"] = false.into();
    yaml["geox-url"] = json!({"mmdb":source});
    if source == MMDB_URL {
        // The bundled rulesets already use IP-ASN. Mihomo expects a dedicated
        // ASN database alongside the inherited Country MMDB.
        yaml["geox-url"]["asn"] = ASN_URL.into();
    }
    yaml["geo-auto-update"] = (!disabled).into();
    yaml["geo-update-interval"] = 168.into();
    Ok(())
}

fn provider_stem(url: &str) -> Option<&str> {
    let suffix = url.strip_prefix(RAW).or_else(|| url.strip_prefix(CDN))?;
    let suffix = suffix.strip_prefix("RULE-SET/")?;
    let name = suffix
        .strip_prefix("github/")
        .unwrap_or(suffix)
        .strip_suffix(".list")?;
    (!name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
    .then_some(name)
}

fn expand_inline<'a>(
    name: &'a str,
    policy: &str,
    no_resolve: bool,
    inline: &BTreeMap<&'a str, Vec<&'a str>>,
    active: &mut BTreeSet<&'a str>,
    output: &mut Vec<String>,
) -> Result<(), VaultError> {
    if active.len() >= 8 || !active.insert(name) {
        return Err(VaultError::Format);
    }
    let rules = inline.get(name).ok_or(VaultError::Format)?;
    for rule in rules {
        let mut parts = fields(rule);
        let kind = parts.next().ok_or(VaultError::Format)?;
        let value = parts.next().ok_or(VaultError::Format)?;
        let no_resolve = no_resolve || parts.any(|field| field == "no-resolve");
        if kind == "RULE-SET" && !matches!(value, "LAN" | "SYSTEM") && inline.contains_key(value) {
            expand_inline(value, policy, no_resolve, inline, active, output)?;
        } else {
            if output.len() >= 4096 {
                return Err(VaultError::TooLarge);
            }
            let modifiers = if no_resolve { ",no-resolve" } else { "" };
            output.push(format!("{kind},{value},{policy}{modifiers}"));
        }
    }
    active.remove(name);
    Ok(())
}

fn yaml_policy<'a>(name: &'a str, groups: &BTreeMap<&str, Group<'_>>) -> Option<&'a str> {
    match name {
        "DIRECT" | "REJECT" => Some(name),
        "REJECT-DROP" | "REJECT-NO-DROP" => Some("REJECT"),
        _ => groups.contains_key(name).then_some(name),
    }
}

fn sing_policy(name: &str) -> &str {
    if name == "DIRECT" { "direct" } else { name }
}

fn sing_action(mut rule: Value, policy: &str) -> Value {
    if policy == "REJECT" {
        rule["action"] = json!("reject");
    } else {
        rule["action"] = json!("route");
        rule["outbound"] = json!(sing_policy(policy));
    }
    rule
}

fn inline_rule(
    kind: &str,
    value: &str,
    policy: &str,
    no_resolve: bool,
    yaml: &mut Vec<String>,
    sing: &mut Vec<Value>,
    aio_providers: Option<&mut BTreeMap<&str, Value>>,
) -> Result<(), VaultError> {
    if kind == "GEOIP" && value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        let options = if no_resolve { ",no-resolve" } else { "" };
        yaml.push(format!("{kind},{value},{policy}{options}"));
        let Some(sing_providers) = aio_providers else {
            return Ok(());
        };
        let (tag, path) = native_sing_geoip_artifact(value)?;
        let url = format!("{RAW}{path}");
        links::validate_url(&url)?;
        let provider = json!({"type":"remote", "tag":tag, "format":"binary", "url":url});
        if let Some(existing) = sing_providers.get(tag) {
            if existing != &provider {
                return Err(VaultError::Format);
            }
        } else {
            sing_providers.insert(tag, provider);
        }
        // Keep the source's country fallback in place, rather than merging its
        // addresses into an earlier domain/service provider with another policy.
        sing.push(sing_action(json!({"rule_set":[tag]}), policy));
        return Ok(());
    }
    if value.is_empty()
        || value.contains([',', '\\'])
        || (value.contains('/') && !matches!(kind, "IP-CIDR" | "IP-CIDR6"))
        || value.chars().any(char::is_control)
    {
        return Ok(());
    }
    let (kind, field, match_value) = match kind {
        "DOMAIN" => (kind, "domain", json!([value])),
        "DOMAIN-SUFFIX" => (kind, "domain_suffix", json!([value])),
        "DOMAIN-KEYWORD" => (kind, "domain_keyword", json!([value])),
        "IP-CIDR" | "IP-CIDR6" => (kind, "ip_cidr", json!([value])),
        "PROCESS-NAME" => (kind, "process_name", json!([value])),
        "SRC-PORT" => {
            let Ok(port) = value.parse::<u16>() else {
                return Ok(());
            };
            (kind, "source_port", json!([port]))
        }
        "PROTOCOL" if matches!(value, "TCP" | "UDP" | "tcp" | "udp") => {
            ("NETWORK", "network", json!([value.to_ascii_lowercase()]))
        }
        _ => return Ok(()),
    };
    let yaml_value = if kind == "NETWORK" {
        match_value[0].as_str().unwrap_or(value)
    } else {
        value
    };
    let options = if no_resolve && matches!(kind, "IP-CIDR" | "IP-CIDR6") {
        ",no-resolve"
    } else {
        ""
    };
    yaml.push(format!("{kind},{yaml_value},{policy}{options}"));
    sing.push(sing_action(json!({(field):match_value}), policy));
    Ok(())
}

fn native_sing_geoip_artifact(country: &str) -> Result<(&'static str, &'static str), VaultError> {
    if country.len() != 2 || !country.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(VaultError::Format);
    }
    links::artifact_paths()
        .find_map(|path| {
            let relative = path.strip_prefix("rulesets/")?;
            let tag = relative
                .strip_prefix("SingBox/")?
                .strip_suffix("_Singbox.srs")?;
            tag.strip_prefix("GeoIP_")?
                .eq_ignore_ascii_case(country)
                .then_some((tag, relative))
        })
        .ok_or(VaultError::MissingArtifact)
}

fn provider_url(stem: &str, sing: bool) -> Result<Option<String>, VaultError> {
    let artifact = if stem.ends_with("_extra") || (sing && stem.ends_with("_ip")) {
        format!("{stem}_supported")
    } else {
        stem.to_owned()
    };
    let url = if sing {
        format!("{RAW}SingBox/{artifact}_Singbox.srs")
    } else {
        format!("{RAW}Mihomo/{artifact}.yaml")
    };
    match links::validate_url(&url) {
        Ok(()) => Ok(Some(url)),
        // Some Surge-only extra matchers have no equivalent published artifact.
        Err(_) if stem.ends_with("_extra") => Ok(None),
        Err(error) => Err(error),
    }
}

// Native advertising shards remain a sing-box companion behavior only. FlClash
// routing is projected exclusively from the Surge source and uses native modules
// for its separately selected preview advertising functionality.
fn native_sing_adblock_artifacts() -> impl Iterator<Item = (&'static str, &'static str)> {
    links::artifact_paths().filter_map(|path| {
        let relative = path.strip_prefix("rulesets/")?;
        let artifact = relative
            .strip_prefix("AdBlock/")?
            .strip_suffix("_Singbox.srs")?;
        let stem = artifact.strip_suffix("_supported").unwrap_or(artifact);
        let shard = stem.strip_prefix("AdBlock_")?;
        // Only compiled ad-block shards: no raw Surge-only rules, HTTPDNS or firewall providers.
        (!shard.is_empty()
            && shard
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
        .then_some((stem, relative))
    })
}

fn add_native_sing_adblock(
    sing_rules: &mut Vec<Value>,
    sing_providers: &mut BTreeMap<&str, Value>,
    insert_at: usize,
) -> Result<(), VaultError> {
    let mut tags = Vec::new();
    for (tag, path) in native_sing_adblock_artifacts() {
        let url = format!("{RAW}{path}");
        links::validate_url(&url)?;
        if sing_providers
            .insert(
                tag,
                json!({
                    "type":"remote", "tag":tag, "format":"binary", "url":url
                }),
            )
            .is_some()
        {
            return Err(VaultError::Format);
        }
        tags.push(tag);
    }
    if tags.is_empty() {
        return Err(VaultError::MissingArtifact);
    }
    sing_rules.insert(
        insert_at,
        json!({"rule_set":tags, "action":"reject", "no_drop":true}),
    );
    Ok(())
}

pub(super) fn derive(surge: &Template) -> Result<Vec<Template>, VaultError> {
    if surge.client != Client::Surge {
        return Err(VaultError::Format);
    }
    let mut groups = BTreeMap::new();
    let mut group_order = Vec::new();
    let mut source_rules = Vec::new();
    let mut inline = BTreeMap::<&str, Vec<&str>>::new();
    let mut general = BTreeMap::new();
    let mut section = "";
    for line in surge.content.lines() {
        let line = inline_comment(line).0.trim();
        if line.is_empty() || line.starts_with(['#', ';']) || line.starts_with("//") {
            continue;
        }
        if let Some(header) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = header;
        } else if section.eq_ignore_ascii_case("General") {
            if let Some((key, value)) = line.split_once('=') {
                general.insert(key.trim(), value.trim());
            }
        } else if section.eq_ignore_ascii_case("Proxy Group") {
            let (name, group) = group(line)?;
            if groups.insert(name, group).is_some() {
                return Err(VaultError::Format);
            }
            group_order.push(name);
        } else if section.eq_ignore_ascii_case("Rule") {
            source_rules.push(line);
        } else if section
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Ruleset "))
        {
            inline.entry(section[8..].trim()).or_default().push(line);
        }
    }

    let global_test_timeout = general
        .get("test-timeout")
        .map(|value| integer(value))
        .transpose()?
        .unwrap_or(5);
    let probe = endpoint(
        general
            .get("proxy-test-url")
            .copied()
            .unwrap_or("https://connectivitycheck.grapheneos.network/generate_204")
            .trim_matches('"'),
        false,
    )?;
    let mut slots = Map::new();
    let mut provider_checks = BTreeMap::<String, Value>::new();
    let mut shared_check_intervals = false;
    let mut yaml_groups = Vec::new();
    let mut outbounds = vec![
        json!({"type":"direct", "tag":"direct"}),
        json!({"type":"socks", "tag":LOCAL_PROXY, "server":"127.0.0.1", "server_port":1080, "version":"5"}),
    ];
    let mut graph = BTreeMap::new();
    for name in group_order {
        let group = groups.get(name).ok_or(VaultError::Format)?;
        let mut providers = Vec::new();
        let mut proxies = Vec::new();
        include_members(
            name,
            &groups,
            &mut BTreeSet::new(),
            &mut providers,
            &mut proxies,
        )?;
        // Surge smart ignores built-ins and nested groups; preserving them as
        // url-test choices could accidentally select a formerly ignored DIRECT.
        if group.kind == GroupKind::Smart {
            proxies.clear();
        }
        let mut uses = Vec::new();
        for slot in providers {
            let key = slot.trim_matches(['{', '}']).to_ascii_lowercase();
            slots.insert(
                key.clone(),
                json!({"type":"http", "url":slot, "interval":86400}),
            );
            uses.push(key);
        }
        graph.insert(
            name,
            proxies
                .iter()
                .copied()
                .filter(|name| groups.contains_key(name))
                .collect(),
        );
        let mut yaml =
            json!({"name":name, "type":group.kind.yaml_type(), "empty-fallback":"REJECT"});
        if group.kind != GroupKind::Select {
            // Surge's legacy group url= is ignored in current releases.
            yaml["url"] = json!(probe);
            let interval = if group.kind == GroupKind::Smart {
                300
            } else {
                group.interval.unwrap_or(600)
            };
            yaml["interval"] = json!(interval);
            let milliseconds = group
                .test_timeout
                .unwrap_or(global_test_timeout)
                .checked_mul(1000)
                .ok_or(VaultError::Format)?;
            yaml["timeout"] = json!(milliseconds);
            // Mihomo's group URL only tests explicit proxies, not provider nodes.
            // Keep shared providers deduplicated and test them at their own level.
            for key in &uses {
                let check = json!({"enable":true, "url":probe, "interval":interval, "timeout":milliseconds, "lazy":true});
                if let Some(previous) = provider_checks.get_mut(key) {
                    if previous["timeout"] != check["timeout"] {
                        return Err(VaultError::Format);
                    }
                    let old_interval = previous["interval"].as_u64().ok_or(VaultError::Format)?;
                    if old_interval != interval {
                        shared_check_intervals = true;
                        previous["interval"] = json!(match (old_interval, interval) {
                            (0, current) => current,
                            (previous, 0) => previous,
                            (previous, current) => previous.min(current),
                        });
                    }
                } else {
                    provider_checks.insert(key.clone(), check);
                }
            }
            if matches!(group.kind, GroupKind::UrlTest | GroupKind::Smart) {
                yaml["tolerance"] = json!(group.tolerance.unwrap_or(100));
            }
            if group.kind == GroupKind::LoadBalance {
                yaml["strategy"] = json!(if group.persistent.unwrap_or(false) {
                    "consistent-hashing"
                } else {
                    "round-robin"
                });
            }
        }
        if let Some(hidden) = group.hidden {
            yaml["hidden"] = json!(hidden);
        }
        if !uses.is_empty() {
            yaml["use"] = json!(uses);
        }
        if !proxies.is_empty() {
            yaml["proxies"] = json!(proxies);
        } else if uses.is_empty() {
            yaml["proxies"] = json!(["REJECT"]);
        }
        // An unsupported expression fails above rather than broadening the group.
        if let Some(filter) = group.filter {
            yaml["filter"] = json!(filter);
        }
        yaml_groups.push(yaml);
        let mut outbound_tags: Vec<_> = proxies
            .iter()
            .filter(|proxy| **proxy != "REJECT")
            .map(|proxy| sing_policy(proxy))
            .collect();
        if !uses.is_empty() || outbound_tags.is_empty() {
            outbound_tags.push(LOCAL_PROXY);
        }
        outbounds.push(json!({"type":"selector", "tag":name, "default":outbound_tags[0], "outbounds":outbound_tags}));
    }
    validate_group_graph(&graph)?;
    for (key, check) in provider_checks {
        slots.get_mut(&key).ok_or(VaultError::Format)?["health-check"] = check;
    }
    if slots.is_empty() {
        return Err(VaultError::Format);
    }

    let mut yaml_rules = Vec::new();
    let mut yaml_providers = Map::new();
    let mut sing_rules = Vec::new();
    let mut sing_providers = BTreeMap::new();
    let uses_aio = general
        .get("geoip-maxmind-url")
        .is_some_and(|source| source.trim_matches('"') == MMDB_URL);
    let mut sing_adblock_at = None;
    let mut final_policy = None;
    let mut expanded = Vec::new();
    for line in source_rules {
        let mut parts = fields(line);
        if parts.next() == Some("RULE-SET")
            && let Some(name) = parts
                .next()
                .filter(|name| !matches!(*name, "LAN" | "SYSTEM") && inline.contains_key(name))
        {
            let policy = parts.next().ok_or(VaultError::Format)?;
            let no_resolve = parts.any(|field| field == "no-resolve");
            expand_inline(
                name,
                policy,
                no_resolve,
                &inline,
                &mut BTreeSet::new(),
                &mut expanded,
            )?;
        } else {
            expanded.push(line.to_owned());
        }
    }
    for line in &expanded {
        let mut fields = fields(line);
        match fields.next() {
            Some("RULE-SET") => {
                let url = fields.next().ok_or(VaultError::Format)?;
                let policy = yaml_policy(fields.next().ok_or(VaultError::Format)?, &groups)
                    .ok_or(VaultError::Format)?;
                let no_resolve = fields.any(|field| field == "no-resolve");
                let Some(stem) = provider_stem(url) else {
                    let rules = match url {
                        "LAN" => LAN,
                        // Surge's internal OS-traffic classifier has no portable equivalent.
                        "SYSTEM" => &[],
                        _ => return Err(VaultError::Format),
                    };
                    for rule in rules {
                        let mut parts = self::fields(rule);
                        if let (Some(kind), Some(value)) = (parts.next(), parts.next()) {
                            let no_resolve = no_resolve || parts.any(|field| field == "no-resolve");
                            inline_rule(
                                kind,
                                value,
                                policy,
                                no_resolve,
                                &mut yaml_rules,
                                &mut sing_rules,
                                None,
                            )?;
                        }
                    }
                    continue;
                };
                // Retain sing-box's existing native advertising position after
                // explicit exceptions. FlClash adds no rules beyond its source.
                if !matches!(
                    stem,
                    "Profile_Exceptions" | "Apple_extra" | "Microsoft_direct"
                ) && sing_adblock_at.is_none()
                {
                    sing_adblock_at = Some(sing_rules.len());
                }
                if let Some(url) = provider_url(stem, false)? {
                    yaml_providers.insert(stem.into(), json!({
                        "type":"http", "behavior":"classical", "format":"yaml", "url":url, "interval":86400
                    }));
                    let options = if no_resolve { ",no-resolve" } else { "" };
                    yaml_rules.push(format!("RULE-SET,{stem},{policy}{options}"));
                }
                if let Some(url) = provider_url(stem, true)? {
                    sing_providers.insert(
                        stem,
                        json!({
                            "type":"remote", "tag":stem, "format":"binary", "url":url
                        }),
                    );
                    sing_rules.push(sing_action(json!({"rule_set":[stem]}), policy));
                }
            }
            Some("FINAL") => {
                let policy = yaml_policy(fields.next().ok_or(VaultError::Format)?, &groups)
                    .ok_or(VaultError::Format)?;
                if final_policy.replace(policy).is_some() {
                    return Err(VaultError::Format);
                }
            }
            Some(kind) => {
                if let (Some(value), Some(policy)) = (
                    fields.next(),
                    fields.next().and_then(|name| yaml_policy(name, &groups)),
                ) {
                    let no_resolve = fields.any(|field| field == "no-resolve");
                    inline_rule(
                        kind,
                        value,
                        policy,
                        no_resolve,
                        &mut yaml_rules,
                        &mut sing_rules,
                        uses_aio.then_some(&mut sing_providers),
                    )?;
                }
            }
            None => {}
        }
    }
    let final_policy = final_policy.ok_or(VaultError::Format)?;
    let sing_adblock_at = sing_adblock_at.unwrap_or(sing_rules.len());
    add_native_sing_adblock(&mut sing_rules, &mut sing_providers, sing_adblock_at)?;
    yaml_rules.push(format!("MATCH,{final_policy}"));
    if final_policy == "REJECT" {
        sing_rules.push(json!({"action":"reject"}));
    }
    let dns = derived_dns(&general)?;
    let ipv6 = dns["ipv6"].as_bool().ok_or(VaultError::Format)?;
    let mut yaml = json!({
        "mode":"rule", "allow-lan":false, "ipv6":ipv6, "dns":dns,
        "proxy-providers":slots, "proxy-groups":yaml_groups,
        "rule-providers":yaml_providers, "rules":yaml_rules
    });
    inherit_geoip(&general, &mut yaml)?;
    let sing = json!({
        "inbounds":[{"type":"mixed", "tag":"local", "listen":"127.0.0.1", "listen_port":7890}],
        "outbounds":outbounds,
        "http_clients":[{"tag":DOWNLOAD_CLIENT, "detour":"direct"}],
        "route":{
            "rules":sing_rules,
            "rule_set":sing_providers.into_values().collect::<Vec<_>>(),
            "final":if final_policy == "REJECT" { LOCAL_PROXY } else { sing_policy(final_policy) },
            "default_http_client":DOWNLOAD_CLIENT,
        }
    });
    let mut yaml_content = String::from(
        "# Derived from the current bundled Surge profile; group/rule order and subscription slots follow that source.\n# DNS is a portable HTTPS-only profile baseline; no system DNS settings are changed.\n# Encrypted DNS follows Surge's DIRECT default unless the source explicitly enables encrypted-dns-follow-outbound-mode.\n",
    );
    if shared_check_intervals {
        yaml_content.push_str("# Shared provider health checks use the shortest requested interval (nonzero when any group schedules checks); their nodes remain deduplicated.\n");
    }
    for (name, group) in &groups {
        if group.kind == GroupKind::Smart {
            yaml_content.push_str(&format!("# Group {name}: Surge smart is approximated by url-test; learned per-site selection/retry is not reproduced; built-in/group members remain ignored as in Surge.\n"));
        }
        if group.kind == GroupKind::LoadBalance {
            yaml_content.push_str(&format!("# Group {name}: load-balance uses Mihomo hashing/round-robin, not Surge random selection.\n"));
        }
        if !group.source_only.is_empty() {
            yaml_content.push_str(&format!(
                "# Group {name}: source-only options not represented: {}.\n",
                group.source_only.join(", ")
            ));
        }
    }
    yaml_content.push_str(&serde_saphyr::to_string(&yaml).map_err(|_| VaultError::Format)?);
    Ok(vec![
        Template {
            name: "FlClash-Surge.yaml".into(),
            client: Client::FlClash,
            content: yaml_content,
        },
        Template {
            name: "sing-box.json".into(),
            client: Client::SingBox,
            content: serde_json::to_string_pretty(&sing).map_err(|_| VaultError::Format)?,
        },
    ])
}

fn validate_group_graph(graph: &BTreeMap<&str, Vec<&str>>) -> Result<(), VaultError> {
    fn visit<'a>(
        name: &'a str,
        graph: &BTreeMap<&'a str, Vec<&'a str>>,
        active: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result<(), VaultError> {
        if done.contains(name) {
            return Ok(());
        }
        if !active.insert(name) {
            return Err(VaultError::Format);
        }
        for &member in graph.get(name).ok_or(VaultError::Format)? {
            visit(member, graph, active, done)?;
        }
        active.remove(name);
        done.insert(name);
        Ok(())
    }
    let mut done = BTreeSet::new();
    for &name in graph.keys() {
        visit(name, graph, &mut BTreeSet::new(), &mut done)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aio_country_fallbacks_keep_source_order_and_policy_in_both_projections() {
        let source = Template {
            name: "Synthetic AIO routing".into(),
            client: Client::Surge,
            content: format!(
                "[General]\ngeoip-maxmind-url = {}\n[Proxy Group]\nProxy = select, policy-path={{{{SUBSCRIPTION_1}}}}\n[Rule]\nDOMAIN,service.example.test,Proxy\nGEOIP,US,Proxy\nGEOIP,JP,Proxy\nGEOIP,TW,Proxy\nGEOIP,CN,DIRECT,no-resolve\nFINAL,Proxy\n",
                crate::pipeline::geo_rulesets::MMDB_URL
            ),
        };
        let output = derive(&source).unwrap();
        let flclash: Value = serde_saphyr::from_str(&output[0].content).unwrap();
        assert_eq!(
            flclash["rules"],
            json!([
                "DOMAIN,service.example.test,Proxy",
                "GEOIP,US,Proxy",
                "GEOIP,JP,Proxy",
                "GEOIP,TW,Proxy",
                "GEOIP,CN,DIRECT,no-resolve",
                "MATCH,Proxy"
            ])
        );
        let sing: Value = serde_json::from_str(&output[1].content).unwrap();
        let rules = sing["route"]["rules"].as_array().unwrap();
        let (adblock, fallback) = rules.split_last().unwrap();
        assert_eq!(adblock["action"], "reject");
        assert_eq!(fallback.len(), 5);
        assert_eq!(fallback[0]["domain"], json!(["service.example.test"]));
        let providers = sing["route"]["rule_set"].as_array().unwrap();
        for (rule, (country, policy)) in fallback[1..].iter().zip([
            ("US", "Proxy"),
            ("JP", "Proxy"),
            ("TW", "Proxy"),
            ("CN", "direct"),
        ]) {
            let tag = format!("GeoIP_{country}");
            assert_eq!(rule["rule_set"], json!([tag]));
            assert_eq!(rule["outbound"], policy);
            let provider = providers.iter().find(|item| item["tag"] == tag).unwrap();
            assert_eq!(provider["url"], format!("{RAW}SingBox/{tag}_Singbox.srs"));
        }
        assert_eq!(
            native_sing_geoip_artifact("ZZ"),
            Err(VaultError::MissingArtifact)
        );
    }

    fn geoip_fixture(general: &str) -> Template {
        Template {
            name: "Synthetic GeoIP profile".into(),
            client: Client::Surge,
            content: format!(
                "[General]\n{general}\n[Proxy Group]\nProxy = select, policy-path={{{{SUBSCRIPTION_1}}}}\n[Rule]\nGEOIP,CN,DIRECT\nFINAL,Proxy\n"
            ),
        }
    }

    #[test]
    fn geoip_source_is_inherited_as_mmdb_and_preserves_auto_update_opt_out() {
        let url = "https://geoip.example.test/country.mmdb";
        for (setting, enabled) in [
            ("", true),
            ("disable-geoip-db-auto-update = false", true),
            ("disable-geoip-db-auto-update = true", false),
        ] {
            let source = geoip_fixture(&format!("geoip-maxmind-url = {url}\n{setting}"));
            let projections = derive(&source).unwrap();
            let yaml: Value = serde_saphyr::from_str(&projections[0].content).unwrap();
            assert_eq!(yaml["geodata-mode"], false);
            assert_eq!(yaml["geox-url"], json!({"mmdb":url}));
            assert_eq!(yaml["geo-auto-update"], enabled);
            assert_eq!(yaml["geo-update-interval"], 168);
            super::super::flclash::subscription_slots(&projections[0]).unwrap();
            let sing: Value = serde_json::from_str(&projections[1].content).unwrap();
            assert!(sing["route"].get("geoip").is_none());
            assert!(sing.get("geox-url").is_none());
            assert!(!projections[1].content.contains(url));
            assert!(!projections[1].content.contains("GeoIP_CN"));
        }
    }

    #[test]
    fn invalid_geoip_source_or_update_flag_fails_without_exporting_a_bad_profile() {
        for url in [
            "http://geoip.example.test/country.mmdb",
            "https://fixture-user:fixture-pass@geoip.example.test/country.mmdb",
            "https://geoip.example.test/country.mmdb?token=fixture-secret",
            "https://geoip.example.test/country.mmdb#fragment",
            "https://geoip.example.test/country.tar.gz",
            "https://geoip.example.test/country.TGZ",
            "https://geoip.example.test/country.ZiP",
            "file:///country.mmdb",
            "",
        ] {
            let error = match derive(&geoip_fixture(&format!("geoip-maxmind-url = {url}"))) {
                Ok(_) => panic!("unsafe source must not produce a profile"),
                Err(error) => error,
            };
            assert!(matches!(error, VaultError::Format));
            assert!(!error.to_string().contains("geoip.example.test"));
        }
        let source = geoip_fixture(
            "geoip-maxmind-url = https://geoip.example.test/country.mmdb\ndisable-geoip-db-auto-update = maybe",
        );
        assert!(matches!(derive(&source), Err(VaultError::Format)));
    }

    #[test]
    fn profiles_without_a_custom_geoip_url_keep_default_geo_settings() {
        for general in ["", "disable-geoip-db-auto-update = true"] {
            let projections = derive(&geoip_fixture(general)).unwrap();
            let yaml: Value = serde_saphyr::from_str(&projections[0].content).unwrap();
            for key in [
                "geodata-mode",
                "geox-url",
                "geo-auto-update",
                "geo-update-interval",
            ] {
                assert!(yaml.get(key).is_none());
            }
        }
    }

    #[test]
    fn nested_profile_sets_keep_providers_policy_and_order() {
        let surge = Template {
            name: "Nested fixture".into(),
            client: Client::Surge,
            content: format!(
                "[Proxy Group]\nProxy = select, policy-path={{{{SUBSCRIPTION_1}}}}\n[Rule]\nRULE-SET,Direct,DIRECT\nRULE-SET,Regional,Proxy,no-resolve\nFINAL,Proxy\n[Ruleset Direct]\nRULE-SET,{RAW}RULE-SET/Profile_Exceptions.list\nRULE-SET,{RAW}RULE-SET/Microsoft_direct.list\n[Ruleset Regional]\nRULE-SET,LAN\nRULE-SET,Services\n[Ruleset Services]\nRULE-SET,{RAW}RULE-SET/JP.list\nDOMAIN,fixture.example\nRULE-SET,{RAW}RULE-SET/Google_ip.list\n"
            ),
        };
        let exports = derive(&surge).unwrap();
        let yaml: Value = serde_saphyr::from_str(&exports[0].content).unwrap();
        let rules = yaml["rules"].as_array().unwrap();
        assert_eq!(rules[0], "RULE-SET,Profile_Exceptions,DIRECT");
        assert_eq!(rules[1], "RULE-SET,Microsoft_direct,DIRECT");
        let jp = rules
            .iter()
            .position(|rule| rule == "RULE-SET,JP,Proxy,no-resolve")
            .unwrap();
        assert!(
            !rules
                .iter()
                .any(|rule| rule.as_str().unwrap().starts_with("RULE-SET,AdBlock_"))
        );
        assert!(
            rules
                .iter()
                .position(|rule| rule == "DOMAIN-SUFFIX,local,Proxy")
                .unwrap()
                < jp
        );
        assert_eq!(
            yaml["rule-providers"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["Profile_Exceptions", "Microsoft_direct", "JP", "Google_ip"])
        );
        assert_eq!(rules[jp + 1], "DOMAIN,fixture.example,Proxy");
        assert_eq!(rules[jp + 2], "RULE-SET,Google_ip,Proxy,no-resolve");
        let sing: Value = serde_json::from_str(&exports[1].content).unwrap();
        assert!(
            sing["route"]["rule_set"]
                .as_array()
                .unwrap()
                .iter()
                .any(|provider| provider["tag"] == "JP")
        );
        let mut cyclic = surge;
        cyclic.content.push_str("RULE-SET,Regional\n");
        assert!(matches!(derive(&cyclic), Err(VaultError::Format)));
    }

    #[test]
    fn flclash_keeps_only_source_routes_while_sing_box_retains_native_adblock() {
        let surge = Template {
            name: "Fixture Surge".into(),
            client: Client::Surge,
            content: format!(
                "[Proxy Group]\nProvider = smart, policy-path={{{{SUBSCRIPTION_1}}}}\nRegional = select, include-other-group=\"Provider\"\n[Rule]\nRULE-SET,{RAW}RULE-SET/Microsoft_direct.list,DIRECT\nRULE-SET,LAN,DIRECT\nDOMAIN,pinned.example,DIRECT\nRULE-SET,{RAW}RULE-SET/Microsoft_proxy.list,Regional\nRULE-SET,{RAW}RULE-SET/GlobalProxy.list,Regional\nFINAL,Regional\n"
            ),
        };
        let output = derive(&surge).unwrap();
        assert_eq!(
            super::super::subscription_slots(&output[0]),
            ["SUBSCRIPTION_1"]
        );
        let yaml: Value = serde_saphyr::from_str(&output[0].content).unwrap();
        let yaml_rules = yaml["rules"].as_array().unwrap();
        let pinned = yaml_rules
            .iter()
            .position(|rule| rule == "DOMAIN,pinned.example,DIRECT")
            .unwrap();
        assert_eq!(yaml_rules[0], "RULE-SET,Microsoft_direct,DIRECT");
        assert_eq!(yaml_rules[1], "DOMAIN-SUFFIX,local,DIRECT");
        let service = yaml_rules
            .iter()
            .position(|rule| rule == "RULE-SET,Microsoft_proxy,Regional")
            .unwrap();
        assert_eq!(
            service,
            pinned + 1,
            "no extra FlClash advertising rules are inserted"
        );
        assert_eq!(yaml_rules[service + 1], "RULE-SET,GlobalProxy,Regional");
        assert_eq!(yaml_rules[service + 2], "MATCH,Regional");
        assert!(
            !yaml_rules
                .iter()
                .any(|rule| rule.as_str().unwrap().starts_with("RULE-SET,AdBlock_"))
        );
        assert_eq!(
            yaml["rule-providers"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["Microsoft_direct", "Microsoft_proxy", "GlobalProxy"])
        );
        let value: Value = serde_json::from_str(&output[1].content).unwrap();
        let sing_rules = value["route"]["rules"].as_array().unwrap();
        assert_eq!(sing_rules[0]["outbound"], "direct");
        assert_eq!(sing_rules[1]["domain_suffix"], json!(["local"]));
        assert_eq!(sing_rules[pinned]["domain"], json!(["pinned.example"]));
        assert_eq!(sing_rules[pinned + 1]["action"], "reject");
        assert_eq!(sing_rules[pinned + 2]["outbound"], "Regional");
        assert_eq!(sing_rules[pinned + 3]["rule_set"], json!(["GlobalProxy"]));
        assert_eq!(value["route"]["final"], "Regional");
        assert_eq!(value["outbounds"][3]["default"], LOCAL_PROXY);

        let catalog: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../rulesets/AdBlock/catalog.json"
        )))
        .unwrap();
        let expected: BTreeSet<_> = catalog["shards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                entry["file"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("rulesets/AdBlock/")
                    .unwrap()
                    .strip_suffix(".list")
                    .unwrap()
            })
            // These shards are URL-regex or user-agent only, not native routing rules.
            .filter(|tag| {
                !matches!(
                    *tag,
                    "AdBlock_Local_other_extra" | "AdBlock_Security_other_extra"
                )
            })
            .collect();
        let sing_tags = sing_rules[pinned + 1]["rule_set"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tag| tag.as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(sing_tags, expected);
        for tag in expected {
            let provider = value["route"]["rule_set"]
                .as_array()
                .unwrap()
                .iter()
                .find(|provider| provider["tag"] == tag)
                .unwrap();
            links::validate_url(provider["url"].as_str().unwrap()).unwrap();
        }
    }

    fn projection_fixture(body: &str) -> Template {
        Template {
            name: "Synthetic inherited Surge".into(),
            client: Client::Surge,
            content: body.into(),
        }
    }

    #[test]
    fn source_group_types_order_members_and_native_parameters_are_preserved() {
        let source = projection_fixture(concat!(
            "[General]\nproxy-test-url=https://probe.example.invalid/check\ntest-timeout=7\n",
            "ipv6=false\nencrypted-dns-server=https://dns.example.invalid/query\n",
            "[Proxy Group]\nBank = select, DIRECT, policy-path={{SUBSCRIPTION_1}}\n",
            "Mirror = select, REJECT, include-other-group=\"Bank,Bank\"\n",
            "Auto = url-test, Mirror, interval=45, tolerance=0, test-timeout=2, url=http://ignored.example.invalid\n",
            "Backup = fallback, Mirror, interval=70, hidden=true\n",
            "Balance = load-balance, Mirror, persistent=true\n",
            "Learned = smart, DIRECT, Mirror, policy-path={{SUBSCRIPTION_2}}, interval=999\n",
            "[Rule]\nFINAL,Auto\n",
        ));
        let output = derive(&source).unwrap();
        let yaml: Value = serde_saphyr::from_str(&output[0].content).unwrap();
        let groups = yaml["proxy-groups"].as_array().unwrap();
        assert_eq!(
            groups
                .iter()
                .map(|group| group["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Bank", "Mirror", "Auto", "Backup", "Balance", "Learned"]
        );
        assert_eq!(groups[1]["proxies"], json!(["REJECT", "DIRECT"]));
        assert_eq!(groups[1]["use"], json!(["subscription_1"]));
        assert_eq!(yaml["proxy-providers"].as_object().unwrap().len(), 2);
        assert_eq!(groups[2]["type"], "url-test");
        assert_eq!(groups[2]["url"], "https://probe.example.invalid/check");
        assert_eq!(groups[2]["interval"], 45);
        assert_eq!(groups[2]["tolerance"], 0);
        assert_eq!(groups[2]["timeout"], 2000);
        assert_eq!(groups[3]["type"], "fallback");
        assert_eq!(groups[3]["hidden"], true);
        assert_eq!(groups[3]["timeout"], 7000);
        assert_eq!(groups[4]["type"], "load-balance");
        assert_eq!(groups[4]["strategy"], "consistent-hashing");
        assert_eq!(groups[5]["type"], "url-test");
        assert_eq!(groups[5]["interval"], 300);
        assert!(
            groups[5].get("proxies").is_none(),
            "Surge smart ignores built-in and nested-group choices"
        );
        assert_eq!(groups[5]["use"], json!(["subscription_2"]));
        let check = &yaml["proxy-providers"]["subscription_2"]["health-check"];
        assert_eq!(check["enable"], true);
        assert_eq!(check["url"], "https://probe.example.invalid/check");
        assert_eq!(check["timeout"], 7000);
        assert_eq!(check["interval"], 300);
        assert!(
            yaml["proxy-providers"]["subscription_1"]
                .get("health-check")
                .is_none()
        );
        assert!(
            output[0]
                .content
                .contains("Surge smart is approximated by url-test")
        );
        assert!(
            output[0]
                .content
                .contains("source-only options not represented: url")
        );
        assert_eq!(yaml["ipv6"], false);
        assert_eq!(yaml["dns"]["ipv6"], false);
        assert_eq!(
            yaml["dns"]["nameserver"],
            json!(["https://dns.example.invalid/query#DIRECT"])
        );
    }

    #[test]
    fn automatic_provider_health_checks_are_shared_without_duplicate_subscriptions() {
        let source = projection_fixture(concat!(
            "[Proxy Group]\nBank = select, policy-path={{SUBSCRIPTION_1}}\n",
            "Auto = url-test, include-other-group=Bank, interval=120\n",
            "Backup = fallback, include-other-group=Bank, interval=300\n",
            "[Rule]\nFINAL,Auto\n",
        ));
        let output = derive(&source).unwrap();
        let yaml: Value = serde_saphyr::from_str(&output[0].content).unwrap();
        assert_eq!(yaml["proxy-providers"].as_object().unwrap().len(), 1);
        assert_eq!(
            yaml["proxy-providers"]["subscription_1"]["health-check"]["interval"],
            120
        );
        assert!(output[0].content.contains("shortest requested interval"));
    }

    #[test]
    fn unknown_group_types_members_filters_and_include_cycles_are_rejected() {
        for body in [
            "Bank = subnet, policy-path={{SUBSCRIPTION_1}}",
            "Bank = select, Missing, policy-path={{SUBSCRIPTION_1}}",
            "Bank = select, policy-path={{SUBSCRIPTION_1}}, policy-regex-filter=\"(?=US)\"",
            "Bank = select, policy-path={{SUBSCRIPTION_1}}, include-other-group=Other\nOther = select, include-other-group=Bank",
        ] {
            let source =
                projection_fixture(&format!("[Proxy Group]\n{body}\n[Rule]\nFINAL,Bank\n"));
            assert!(matches!(derive(&source), Err(VaultError::Format)), "{body}");
        }
    }

    #[test]
    fn included_filtered_groups_are_rejected_instead_of_broadening_nodes() {
        let standalone = projection_fixture(concat!(
            "[Proxy Group]\nNarrow = smart, policy-path={{SUBSCRIPTION_1}}, policy-regex-filter=JP\n",
            "[Rule]\nFINAL,Narrow\n",
        ));
        assert!(derive(&standalone).is_ok());
        let included = projection_fixture(concat!(
            "[Proxy Group]\nNarrow = smart, policy-path={{SUBSCRIPTION_1}}, policy-regex-filter=JP\n",
            "Combined = select, include-other-group=Narrow\n",
            "[Rule]\nFINAL,Combined\n",
        ));
        assert!(matches!(derive(&included), Err(VaultError::Format)));
    }

    #[test]
    fn source_probe_can_be_http_but_dns_must_remain_https() {
        let source = projection_fixture(
            "[General]\nipv6=true\nproxy-test-url=http://probe.example.invalid/check\n[Proxy Group]\nBank = smart, policy-path={{SUBSCRIPTION_1}}\n[Rule]\nFINAL,Bank\n",
        );
        let output = derive(&source).unwrap();
        let yaml: Value = serde_saphyr::from_str(&output[0].content).unwrap();
        assert_eq!(yaml["ipv6"], true);
        assert_eq!(yaml["dns"]["ipv6"], true);
        assert_eq!(
            yaml["proxy-groups"][0]["url"],
            "http://probe.example.invalid/check"
        );
        assert_eq!(
            yaml["proxy-providers"]["subscription_1"]["health-check"]["url"],
            "http://probe.example.invalid/check"
        );
        let insecure = projection_fixture(&source.content.replace(
            "[General]",
            "[General]\nencrypted-dns-server=http://dns.example.invalid/query",
        ));
        assert!(matches!(derive(&insecure), Err(VaultError::Format)));
    }

    #[test]
    fn encrypted_dns_defaults_and_explicit_false_remain_direct_before_nodes_load() {
        for follow in ["", "encrypted-dns-follow-outbound-mode=false\n"] {
            let source = projection_fixture(&format!(
                "[General]\n{follow}encrypted-dns-server=https://dns.example.invalid/query, https://second.example.invalid/query\n[Proxy Group]\nAuto = smart, policy-path={{{{SUBSCRIPTION_1}}}}\n[Rule]\nFINAL,Auto\n"
            ));
            let output = derive(&source).unwrap();
            let yaml: Value = serde_saphyr::from_str(&output[0].content).unwrap();
            let expected = json!([
                "https://dns.example.invalid/query#DIRECT",
                "https://second.example.invalid/query#DIRECT"
            ]);
            assert_eq!(yaml["dns"]["respect-rules"], true);
            assert_eq!(yaml["dns"]["nameserver"], expected);
            assert_eq!(yaml["dns"]["proxy-server-nameserver"], expected);
            assert_eq!(
                yaml["dns"]["default-nameserver"],
                json!(["https://223.5.5.5/dns-query#DIRECT"])
            );
        }
    }

    #[test]
    fn explicit_encrypted_dns_following_keeps_only_bootstrap_and_node_dns_direct() {
        let source = projection_fixture(concat!(
            "[General]\nencrypted-dns-follow-outbound-mode=true\n",
            "encrypted-dns-server=https://dns.example.invalid/query\n",
            "[Proxy Group]\nAuto = smart, policy-path={{SUBSCRIPTION_1}}\n",
            "[Rule]\nFINAL,Auto\n",
        ));
        let output = derive(&source).unwrap();
        let yaml: Value = serde_saphyr::from_str(&output[0].content).unwrap();
        assert_eq!(yaml["dns"]["respect-rules"], true);
        assert_eq!(
            yaml["dns"]["nameserver"],
            json!(["https://dns.example.invalid/query"])
        );
        assert_eq!(
            yaml["dns"]["proxy-server-nameserver"],
            json!(["https://dns.example.invalid/query#DIRECT"])
        );
        assert_eq!(
            yaml["dns"]["default-nameserver"],
            json!(["https://223.5.5.5/dns-query#DIRECT"])
        );
    }

    #[test]
    fn conflicting_shared_provider_timeouts_are_not_silently_collapsed() {
        let source = projection_fixture(
            "[Proxy Group]\nBank = select, policy-path={{SUBSCRIPTION_1}}\nAuto = url-test, include-other-group=Bank, test-timeout=2\nBackup = fallback, include-other-group=Bank, test-timeout=5\n[Rule]\nFINAL,Auto\n",
        );
        assert!(matches!(derive(&source), Err(VaultError::Format)));
    }

    #[test]
    fn derived_https_dns_baseline_allows_the_existing_windows_guard() {
        let source = projection_fixture(concat!(
            "[Proxy Group]\nBank = smart, policy-path={{SUBSCRIPTION_1}}\n",
            "[Rule]\nFINAL,Bank\n",
        ));
        let output = derive(&source).unwrap();
        let subscriptions = BTreeMap::from([(
            "SUBSCRIPTION_1".to_owned(),
            "https://example.invalid/subscription".to_owned(),
        )]);
        let rendered =
            super::super::render_flclash_windows_dns(&output[0], &subscriptions).unwrap();
        let yaml: Value = serde_saphyr::from_str(&rendered).unwrap();
        assert_eq!(yaml["dns"]["enable"], true);
        assert_eq!(yaml["dns"]["respect-rules"], true);
        assert_eq!(
            yaml["dns"]["default-nameserver"],
            json!(["https://223.5.5.5/dns-query#DIRECT"])
        );
        assert_eq!(
            yaml["proxy-providers"]["subscription_1"]["url"],
            "https://example.invalid/subscription"
        );
        assert_eq!(yaml["tun"]["enable"], true);
        assert_eq!(yaml["tun"]["strict-route"], true);
        assert_eq!(yaml["tun"]["dns-hijack"], json!(["any:53", "tcp://any:53"]));
    }
}
