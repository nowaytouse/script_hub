//! Client projections of the unlocked bundled Surge profile, not a general profile converter.

use super::{Client, Template, VaultError, inline_comment, links, slot_ranges};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

const RAW: &str = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/";
const CDN: &str = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/";
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

#[derive(Default)]
struct Group<'a> {
    members: Vec<&'a str>,
    includes: Vec<&'a str>,
    slot: Option<&'a str>,
    filter: Option<&'a str>,
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
    let mut group = Group::default();
    for field in fields(value).skip(1) {
        let Some((key, value)) = field.split_once('=') else {
            group.members.push(field.trim_matches('"'));
            continue;
        };
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "policy-path" => {
                let ranges = slot_ranges(value)?;
                if !matches!(ranges.as_slice(), [(0, end, _)] if *end == value.len())
                    || group.slot.replace(value).is_some()
                {
                    return Err(VaultError::Format);
                }
            }
            "include-other-group" => group.includes.extend(value.split(',').map(str::trim)),
            "policy-regex-filter" => group.filter = Some(value),
            _ => {}
        }
    }
    Ok((name, group))
}

fn include_providers<'a>(
    name: &'a str,
    groups: &BTreeMap<&'a str, Group<'a>>,
    visiting: &mut BTreeSet<&'a str>,
    providers: &mut Vec<&'a str>,
) -> Result<(), VaultError> {
    if !visiting.insert(name) {
        return Err(VaultError::Format);
    }
    let group = groups.get(name).ok_or(VaultError::Format)?;
    for &name in &group.includes {
        include_providers(name, groups, visiting, providers)?;
    }
    if let Some(slot) = group.slot
        && !providers.contains(&slot)
    {
        providers.push(slot);
    }
    visiting.remove(name);
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
) {
    if value.is_empty()
        || value.contains([',', '\\'])
        || (value.contains('/') && !matches!(kind, "IP-CIDR" | "IP-CIDR6"))
        || value.chars().any(char::is_control)
    {
        return;
    }
    let (kind, field, match_value) = match kind {
        "DOMAIN" => (kind, "domain", json!([value])),
        "DOMAIN-SUFFIX" => (kind, "domain_suffix", json!([value])),
        "DOMAIN-KEYWORD" => (kind, "domain_keyword", json!([value])),
        "IP-CIDR" | "IP-CIDR6" => (kind, "ip_cidr", json!([value])),
        "PROCESS-NAME" => (kind, "process_name", json!([value])),
        "SRC-PORT" => {
            let Ok(port) = value.parse::<u16>() else {
                return;
            };
            (kind, "source_port", json!([port]))
        }
        "PROTOCOL" if matches!(value, "TCP" | "UDP" | "tcp" | "udp") => {
            ("NETWORK", "network", json!([value.to_ascii_lowercase()]))
        }
        // sing-box removed native GEOIP; do not broaden these into a different set.
        "GEOIP" if value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_uppercase()) => {
            let options = if no_resolve { ",no-resolve" } else { "" };
            yaml.push(format!("{kind},{value},{policy}{options}"));
            return;
        }
        _ => return,
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

fn native_adblock_artifacts(sing: bool) -> impl Iterator<Item = (&'static str, &'static str)> {
    let (directory, extension) = if sing {
        ("AdBlock/", "_Singbox.srs")
    } else {
        ("AdBlock/Mihomo/", ".yaml")
    };
    links::artifact_paths().filter_map(move |path| {
        let relative = path.strip_prefix("rulesets/")?;
        let artifact = relative.strip_prefix(directory)?.strip_suffix(extension)?;
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

fn add_native_adblock(
    yaml_rules: &mut Vec<String>,
    yaml_providers: &mut Map<String, Value>,
    sing_rules: &mut Vec<Value>,
    sing_providers: &mut BTreeMap<&str, Value>,
    insert_at: (usize, usize),
) -> Result<(), VaultError> {
    let mut rejects = Vec::new();
    for (tag, path) in native_adblock_artifacts(false) {
        let url = format!("{RAW}{path}");
        links::validate_url(&url)?;
        if yaml_providers.insert(tag.into(), json!({
            "type":"http", "behavior":"classical", "format":"yaml", "url":url, "interval":86400
        })).is_some() {
            return Err(VaultError::Format);
        }
        // Prevent even an upstream CIDR/ASN entry without no-resolve from issuing DNS.
        let modifier = if tag.ends_with("_ip") {
            ",no-resolve"
        } else {
            ""
        };
        rejects.push(format!("RULE-SET,{tag},REJECT{modifier}"));
    }
    let mut tags = Vec::new();
    for (tag, path) in native_adblock_artifacts(true) {
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
    if rejects.is_empty() || tags.is_empty() {
        return Err(VaultError::MissingArtifact);
    }
    yaml_rules.splice(insert_at.0..insert_at.0, rejects);
    sing_rules.insert(
        insert_at.1,
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

    let mut slots = Map::new();
    let mut yaml_groups = Vec::new();
    let mut outbounds = vec![
        json!({"type":"direct", "tag":"direct"}),
        json!({"type":"socks", "tag":LOCAL_PROXY, "server":"127.0.0.1", "server_port":1080, "version":"5"}),
    ];
    let mut graph = BTreeMap::new();
    for name in group_order {
        let group = groups.get(name).ok_or(VaultError::Format)?;
        let mut providers = Vec::new();
        include_providers(name, &groups, &mut BTreeSet::new(), &mut providers)?;
        let mut uses = Vec::new();
        for slot in providers {
            let key = slot.trim_matches(['{', '}']).to_ascii_lowercase();
            slots.insert(
                key.clone(),
                json!({"type":"http", "url":slot, "interval":86400}),
            );
            uses.push(key);
        }
        let proxies: Vec<_> = group
            .members
            .iter()
            .filter_map(|member| yaml_policy(member, &groups))
            .collect();
        graph.insert(
            name,
            proxies
                .iter()
                .copied()
                .filter(|name| groups.contains_key(name))
                .collect(),
        );
        let mut yaml = json!({"name":name, "type":"select", "empty-fallback":"REJECT"});
        if !uses.is_empty() {
            yaml["use"] = json!(uses);
        }
        if !proxies.is_empty() {
            yaml["proxies"] = json!(proxies);
        } else if uses.is_empty() {
            yaml["proxies"] = json!(["REJECT"]);
        }
        // A filter outside the portable regex subset cannot safely be copied.
        if let Some(filter) = group
            .filter
            .filter(|filter| regex::Regex::new(filter).is_ok())
        {
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
    if slots.is_empty() {
        return Err(VaultError::Format);
    }

    let mut yaml_rules = Vec::new();
    let mut yaml_providers = Map::new();
    let mut sing_rules = Vec::new();
    let mut sing_providers = BTreeMap::new();
    let mut adblock_at = None;
    let mut final_policy = None;
    for line in source_rules {
        let mut fields = fields(line);
        match fields.next() {
            Some("RULE-SET") => {
                let url = fields.next().ok_or(VaultError::Format)?;
                let policy = yaml_policy(fields.next().ok_or(VaultError::Format)?, &groups)
                    .ok_or(VaultError::Format)?;
                let no_resolve = fields.any(|field| field == "no-resolve");
                let Some(stem) = provider_stem(url) else {
                    let rules = if url == "LAN" {
                        LAN
                    } else {
                        inline.get(url).map_or(&[][..], Vec::as_slice)
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
                            );
                        }
                    }
                    continue;
                };
                // Keep leading local overrides and the explicit Microsoft sign-in exception.
                // Broad service rules (including Direct and GlobalProxy) must not shadow AdBlock.
                if stem != "Microsoft_direct" && adblock_at.is_none() {
                    adblock_at = Some((yaml_rules.len(), sing_rules.len()));
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
                    );
                }
            }
            None => {}
        }
    }
    let final_policy = final_policy.ok_or(VaultError::Format)?;
    let adblock_at = adblock_at.unwrap_or((yaml_rules.len(), sing_rules.len()));
    add_native_adblock(
        &mut yaml_rules,
        &mut yaml_providers,
        &mut sing_rules,
        &mut sing_providers,
        adblock_at,
    )?;
    yaml_rules.push(format!("MATCH,{final_policy}"));
    if final_policy == "REJECT" {
        sing_rules.push(json!({"action":"reject"}));
    }
    let yaml = json!({
        "mode":"rule", "allow-lan":false,
        "proxy-providers":slots, "proxy-groups":yaml_groups,
        "rule-providers":yaml_providers, "rules":yaml_rules
    });
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
    Ok(vec![
        Template {
            name: "FlClash.yaml".into(),
            client: Client::FlClash,
            content: serde_saphyr::to_string(&yaml).map_err(|_| VaultError::Format)?,
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
    fn projects_repository_order_with_native_adblock() {
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
        assert_eq!(yaml_rules[service + 1], "RULE-SET,GlobalProxy,Regional");
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
        let yaml_tags = yaml_rules[pinned + 1..service]
            .iter()
            .map(|rule| {
                let (tag, reject) = rule
                    .as_str()
                    .unwrap()
                    .strip_prefix("RULE-SET,")
                    .unwrap()
                    .split_once(',')
                    .unwrap();
                assert_eq!(
                    reject,
                    if tag.ends_with("_ip") {
                        "REJECT,no-resolve"
                    } else {
                        "REJECT"
                    }
                );
                tag
            })
            .collect::<BTreeSet<_>>();
        let sing_tags = sing_rules[pinned + 1]["rule_set"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tag| tag.as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(yaml_tags, expected);
        assert_eq!(sing_tags, expected);
        for tag in expected {
            let provider = value["route"]["rule_set"]
                .as_array()
                .unwrap()
                .iter()
                .find(|provider| provider["tag"] == tag)
                .unwrap();
            links::validate_url(provider["url"].as_str().unwrap()).unwrap();
            links::validate_url(yaml["rule-providers"][tag]["url"].as_str().unwrap()).unwrap();
        }
    }
}
