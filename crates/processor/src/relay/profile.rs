//! Build a local Mihomo configuration from one reviewed node list.

use std::{collections::HashSet, fmt, path::Path};

use serde_json::{Map, Value};
use zeroize::{Zeroize, Zeroizing};

use super::{
    RelayError,
    export::{OutputFormat, export_subscription},
    model::{MAX_CONTENT, PrivateJson, mihomo},
};

pub const MAX_TEMPLATE_BYTES: usize = MAX_CONTENT;

/// Write a local profile to a new file outside repositories and vault folders.
pub fn write_private_yaml(path: &Path, output: &str) -> Result<(), String> {
    if !path
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
    {
        return Err("Choose a .yaml or .yml file for the Mihomo profile.".into());
    }
    crate::vault::destination::write_private_export(path, output.as_bytes())
}

const RESERVED: [&str; 6] = [
    "DIRECT",
    "REJECT",
    "REJECT-DROP",
    "PASS",
    "PASS-RULE",
    "COMPATIBLE",
];

#[derive(Debug)]
pub enum Error {
    TooLarge,
    InvalidTemplate,
    NoEligibleGroup,
    InvalidGroup,
    NameConflict,
    Export(RelayError),
    InvalidNodes,
    Render,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => f.write_str("The template or generated profile exceeds 4 MiB."),
            Self::InvalidTemplate => f.write_str("The base file is not a supported Mihomo configuration."),
            Self::NoEligibleGroup => f.write_str("Use a select, url-test, fallback, or load-balance group with explicit members or unfiltered automatic inclusion."),
            Self::InvalidGroup => f.write_str("Choose an eligible group from this template."),
            Self::NameConflict => f.write_str("A node or group name conflicts with another name or a built-in policy."),
            Self::Export(error) => error.fmt(f),
            Self::InvalidNodes => f.write_str("The reviewed result does not contain a valid Mihomo node list."),
            Self::Render => f.write_str("Could not encode the combined Mihomo configuration within the supported YAML limits."),
        }
    }
}

impl std::error::Error for Error {}

pub struct MihomoTemplate {
    document: PrivateJson,
    groups: Vec<String>,
}

impl Drop for MihomoTemplate {
    fn drop(&mut self) {
        self.groups.zeroize();
    }
}

impl MihomoTemplate {
    pub fn parse(input: &str) -> Result<Self, Error> {
        if input.len() > MAX_TEMPLATE_BYTES {
            return Err(Error::TooLarge);
        }
        let document = mihomo::parse_yaml_template(input).map_err(|_| Error::InvalidTemplate)?;
        let profile = document.0.as_object().ok_or(Error::InvalidTemplate)?;
        let mut occupied = HashSet::from(RESERVED);
        for node in optional_list(profile, "proxies").ok_or(Error::InvalidTemplate)? {
            let (name, _) = named_item(node).ok_or(Error::InvalidTemplate)?;
            if name == "GLOBAL" || !occupied.insert(name) {
                return Err(Error::NameConflict);
            }
        }

        for key in ["proxy-providers", "rule-providers"] {
            if profile.get(key).is_some_and(|value| {
                !value.is_null()
                    && value.as_object().is_none_or(|providers| {
                        providers
                            .iter()
                            .any(|(name, provider)| !valid_name(name) || !provider.is_object())
                    })
            }) {
                return Err(Error::InvalidTemplate);
            }
        }
        let providers = profile.get("proxy-providers").and_then(Value::as_object);
        string_list(profile, "rules").ok_or(Error::InvalidTemplate)?;

        let mut groups = Zeroizing::new(Vec::new());
        for group in optional_list(profile, "proxy-groups").ok_or(Error::InvalidTemplate)? {
            let (name, kind) = named_item(group).ok_or(Error::InvalidTemplate)?;
            if !occupied.insert(name) {
                return Err(Error::NameConflict);
            }
            let group = group.as_object().ok_or(Error::InvalidTemplate)?;
            string_list(group, "proxies").ok_or(Error::InvalidTemplate)?;
            string_list(group, "use").ok_or(Error::InvalidTemplate)?;
            let has_node_filters = ["filter", "exclude-filter", "exclude-type"]
                .iter()
                .any(|key| match group.get(*key) {
                    None | Some(Value::Null) => false,
                    Some(Value::String(pattern)) => !pattern.is_empty(),
                    _ => true,
                });
            if matches!(kind, "select" | "url-test" | "fallback" | "load-balance")
                && (!automatically_includes_nodes(group) || !has_node_filters)
                // Mihomo creates a provider named after a group with inline nodes.
                && providers.is_none_or(|providers| !providers.contains_key(name))
            {
                groups.push(name.to_owned());
            }
        }
        if groups.is_empty() {
            return Err(Error::NoEligibleGroup);
        }
        Ok(Self {
            document,
            groups: std::mem::take(&mut *groups),
        })
    }

    pub fn groups(&self) -> &[String] {
        &self.groups
    }

    /// Build without reading files, contacting providers, or modifying the template.
    pub fn build(&self, prepared: &str, group: &str) -> Result<Zeroizing<String>, Error> {
        if !self.groups.iter().any(|name| name == group) {
            return Err(Error::InvalidGroup);
        }
        if prepared.len() > MAX_CONTENT {
            return Err(Error::TooLarge);
        }
        let exported =
            export_subscription(prepared, OutputFormat::Mihomo).map_err(Error::Export)?;
        let mut nodes = mihomo::parse_yaml_document(&exported).map_err(|_| Error::InvalidNodes)?;
        let nodes = nodes
            .0
            .get_mut("proxies")
            .and_then(Value::as_array_mut)
            .ok_or(Error::InvalidNodes)?;
        if nodes.is_empty() {
            return Err(Error::InvalidNodes);
        }

        let mut document = PrivateJson(self.document.0.clone());
        let profile = document.0.as_object_mut().ok_or(Error::InvalidTemplate)?;
        let proxy_list = optional_list(profile, "proxies").ok_or(Error::InvalidTemplate)?;
        let groups = optional_list(profile, "proxy-groups").ok_or(Error::InvalidTemplate)?;
        let index = groups
            .iter()
            .position(|item| item.get("name").and_then(Value::as_str) == Some(group))
            .ok_or(Error::InvalidGroup)?;
        let group = groups[index].as_object().ok_or(Error::InvalidGroup)?;
        let automatic = automatically_includes_nodes(group);
        let members = string_list(group, "proxies").ok_or(Error::InvalidGroup)?;

        let mut occupied = HashSet::from(RESERVED);
        occupied.insert("GLOBAL");
        occupied.extend(
            proxy_list
                .iter()
                .chain(groups)
                .filter_map(|item| item.get("name").and_then(Value::as_str)),
        );
        occupied.extend(members.iter().filter_map(Value::as_str));
        for node in nodes.iter() {
            let (name, _) = named_item(node).ok_or(Error::InvalidNodes)?;
            if !occupied.insert(name) {
                return Err(Error::NameConflict);
            }
        }

        // Automatic groups will include the newly declared proxies once.
        if !automatic {
            let members = profile
                .get_mut("proxy-groups")
                .and_then(Value::as_array_mut)
                .and_then(|groups| groups[index].as_object_mut())
                .and_then(|group| list_mut(group, "proxies"))
                .ok_or(Error::InvalidGroup)?;
            members.extend(nodes.iter().filter_map(|node| node.get("name")).cloned());
        }
        list_mut(profile, "proxies")
            .ok_or(Error::InvalidTemplate)?
            .append(nodes);

        let output = mihomo::render(&document.0).map_err(|_| Error::Render)?;
        if output.len() > MAX_TEMPLATE_BYTES {
            return Err(Error::TooLarge);
        }
        // The two accepted inputs can exceed the combined document's YAML budget.
        let roundtrip = mihomo::parse_yaml_document(&output).map_err(|_| Error::Render)?;
        if roundtrip.0 != document.0 {
            return Err(Error::Render);
        }
        Ok(output)
    }
}

fn valid_name(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn named_item(item: &Value) -> Option<(&str, &str)> {
    let name = item.get("name")?.as_str().filter(|name| valid_name(name))?;
    let kind = item.get("type")?.as_str().filter(|kind| valid_name(kind))?;
    Some((name, kind))
}

fn automatically_includes_nodes(group: &Map<String, Value>) -> bool {
    ["include-all", "include-all-proxies"]
        .iter()
        .any(|key| group.get(*key).and_then(Value::as_bool) == Some(true))
}

fn optional_list<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a [Value]> {
    match map.get(key) {
        None | Some(Value::Null) => Some(&[]),
        Some(value) => value.as_array().map(Vec::as_slice),
    }
}

fn string_list<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a [Value]> {
    optional_list(map, key).filter(|items| items.iter().all(Value::is_string))
}

fn list_mut<'a>(map: &'a mut Map<String, Value>, key: &str) -> Option<&'a mut Vec<Value>> {
    let value = map.entry(key).or_insert(Value::Null);
    if value.is_null() {
        *value = Value::Array(Vec::new());
    }
    value.as_array_mut()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TEMPLATE: &str = r#"
mixed-port: 7890
ipv6: true
dns: {enable: true, nameserver: ["https://dns.example.invalid/query"]}
proxies:
  - {name: Existing, type: socks5, server: base.example.invalid, port: 1080}
proxy-providers:
  External: {type: http, url: "https://sub.example.invalid/list.yaml"}
rule-providers:
  Local: {type: inline, behavior: domain, payload: ['+.example.invalid']}
proxy-groups:
  - {name: Select, type: select, proxies: [DIRECT, Existing], use: [External]}
  - {name: Automatic, type: url-test, include-all-proxies: true}
  - {name: Filtered, type: url-test, include-all-proxies: true, filter: Excluded}
  - {name: External, type: select, use: [External]}
  - {name: Failover, type: fallback, proxies: [DIRECT]}
rules: ["RULE-SET,Local,Select", "MATCH,Failover"]
"#;

    const PREPARED: &str = r#"
Alpha = socks5, alpha.example.invalid, 1080
Beta = socks5, beta.example.invalid, 1080
"#;

    #[test]
    fn expands_bounded_template_anchors_and_merge_keys() {
        let template = MihomoTemplate::parse(
            r#"
x-group-defaults: &defaults {type: select, proxies: [DIRECT]}
proxy-groups:
  - <<: *defaults
    name: Pick
rules: ["MATCH,Pick"]
"#,
        )
        .unwrap();
        let output = template.build(PREPARED, "Pick").unwrap();
        let parsed = mihomo::parse_yaml_document(&output).unwrap();
        assert_eq!(
            parsed.0["proxy-groups"][0]["proxies"],
            json!(["DIRECT", "Alpha", "Beta"])
        );
    }

    #[test]
    fn fields_outside_the_selected_group_survive_profile_generation() {
        let template = MihomoTemplate::parse(TEMPLATE).unwrap();
        assert_eq!(template.groups(), &["Select", "Automatic", "Failover"]);
        let output = template.build(PREPARED, "Select").unwrap();
        let parsed = mihomo::parse_yaml_document(&output).unwrap();
        let proxies = parsed.0["proxies"].as_array().unwrap();
        assert_eq!(proxies.len(), 3);
        assert_eq!(proxies[0]["name"], "Existing");
        assert_eq!(proxies[1]["name"], "Alpha");
        assert_eq!(proxies[2]["name"], "Beta");
        assert_eq!(
            parsed.0["proxy-groups"][0]["proxies"],
            json!(["DIRECT", "Existing", "Alpha", "Beta"])
        );
        let original = mihomo::parse_yaml_document(TEMPLATE).unwrap();
        assert_eq!(
            parsed.0["proxy-groups"][0]["use"],
            original.0["proxy-groups"][0]["use"]
        );
        assert_eq!(
            parsed.0["proxy-groups"].as_array().unwrap()[1..],
            original.0["proxy-groups"].as_array().unwrap()[1..]
        );
        for key in [
            "mixed-port",
            "ipv6",
            "dns",
            "proxy-providers",
            "rule-providers",
            "rules",
        ] {
            assert_eq!(parsed.0[key], original.0[key]);
        }
        assert_eq!(template.document.0, original.0);
    }

    #[test]
    fn provider_only_group_can_receive_native_mihomo_nodes() {
        let template = MihomoTemplate::parse(
            "proxy-providers: {External: {type: http, url: 'https://sub.example.invalid/list'}}\n\
             proxy-groups: [{name: Pick, type: select, use: [External]}]\n",
        )
        .unwrap();
        let output = template
            .build(
                "proxies: [{name: Native, type: socks5, server: node.example.invalid, port: 1080}]",
                "Pick",
            )
            .unwrap();
        let parsed = mihomo::parse_yaml_document(&output).unwrap();
        assert_eq!(parsed.0["proxies"][0]["name"], "Native");
        assert_eq!(parsed.0["proxy-groups"][0]["proxies"], json!(["Native"]));
        assert_eq!(parsed.0["proxy-groups"][0]["use"], json!(["External"]));
    }

    #[test]
    fn dynamic_group_includes_new_nodes_without_explicit_duplicate_members() {
        let template = MihomoTemplate::parse(TEMPLATE).unwrap();
        let output = template.build(PREPARED, "Automatic").unwrap();
        let parsed = mihomo::parse_yaml_document(&output).unwrap();
        assert_eq!(parsed.0["proxies"].as_array().unwrap().len(), 3);
        assert_eq!(
            parsed.0["proxy-groups"],
            template.document.0["proxy-groups"]
        );
        assert!(matches!(
            template.build(PREPARED, "Filtered"),
            Err(Error::InvalidGroup)
        ));
        assert!(matches!(
            template.build(PREPARED, "External"),
            Err(Error::InvalidGroup)
        ));
    }

    #[test]
    fn rejects_conflicts_without_exposing_node_names() {
        let template = MihomoTemplate::parse(TEMPLATE).unwrap();
        for name in ["Existing", "Select", "DIRECT", "GLOBAL"] {
            let node = format!(
                "proxies: [{{name: {name}, type: socks5, server: node.example.invalid, port: 1080}}]"
            );
            assert!(matches!(
                template.build(&node, "Select"),
                Err(Error::NameConflict)
            ));
        }
        let duplicate = r#"
proxies:
  - {name: synthetic-private, type: socks5, server: alpha.example.invalid, port: 1080}
  - {name: synthetic-private, type: socks5, server: beta.example.invalid, port: 1080}
"#;
        let error = template.build(duplicate, "Select").unwrap_err();
        assert!(!error.to_string().contains("synthetic-private"));
    }

    #[test]
    fn rejects_unsupported_template_structure() {
        for template in [
            "proxies: []\nproxies: []",
            "proxy-groups: value",
            "proxy-groups: [{name: Pick, type: select, proxies: wrong}]",
            "proxy-groups: [{name: Pick, type: select}]\n---\n{}",
        ] {
            assert!(matches!(
                MihomoTemplate::parse(template),
                Err(Error::InvalidTemplate)
            ));
        }
        assert!(matches!(
            MihomoTemplate::parse(
                "proxy-groups: [{name: Automatic, type: url-test, include-all: true, exclude-type: SOCKS5}]"
            ),
            Err(Error::NoEligibleGroup),
        ));
    }
}
