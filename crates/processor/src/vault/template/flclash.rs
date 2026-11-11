//! A bounded YAML template subset, with subscriptions bound only at HTTP proxy-provider URLs.
//! Comments are deliberately discarded rather than copied into a public template.

use super::{
    Client, MAX_SUBSCRIPTION_BYTES, MAX_TEMPLATE_BYTES, MAX_TEMPLATES, Template, VaultError,
};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use url::Url;
use zeroize::{Zeroize, Zeroizing};

const DNS_RESOLVERS: &[&str] = &[
    "default-nameserver",
    "proxy-server-nameserver",
    "nameserver",
    "fallback",
    "direct-nameserver",
];
const CN_DOH: &str = "https://dns.alidns.com/dns-query#DIRECT";

#[derive(Clone, Copy)]
enum UrlMode {
    Import,
    Slots,
}

// Keep an owned parsed copy out of preferences/logs, and erase its string buffers on drop.
struct PrivateYaml(Value);

fn erase(value: &mut Value) {
    match value {
        Value::String(value) => value.zeroize(),
        Value::Array(values) => values.iter_mut().for_each(erase),
        Value::Object(values) => {
            for (mut key, mut value) in std::mem::take(values) {
                key.zeroize();
                erase(&mut value);
            }
        }
        _ => {}
    }
}

impl Drop for PrivateYaml {
    fn drop(&mut self) {
        erase(&mut self.0);
    }
}

// As in relay/model/mihomo.rs: reject duplicate mappings before converting them to JSON.
struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an unambiguous YAML value")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|number| UniqueValue(Value::Number(number)))
                    .ok_or_else(|| E::custom("non-finite YAML number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut mapping: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some((key, UniqueValue(value))) =
                    mapping.next_entry::<String, UniqueValue>()?
                {
                    if values.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate YAML mapping key"));
                    }
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

fn parse(content: &str) -> Result<PrivateYaml, VaultError> {
    if content.len() > MAX_TEMPLATE_BYTES {
        return Err(VaultError::TooLarge);
    }
    if content.is_empty() || content.contains('\0') {
        return Err(VaultError::Format);
    }
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_documents: 1,
            max_depth: 32,
            flow_nesting_limit: 32,
            max_nodes: 100_000,
            max_events: 200_000,
            max_aliases: 0,
            max_anchors: 0,
            max_recorded_anchor_events: 0,
            max_recorded_anchor_bytes: 0,
            max_total_scalar_bytes: MAX_TEMPLATE_BYTES,
        },
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        merge_keys: serde_saphyr::MergeKeyPolicy::Error,
        reject_unsupported_tags: true,
        strict_booleans: true,
        no_schema: true,
        with_snippet: false,
    };
    let UniqueValue(value) =
        serde_saphyr::from_str_with_options(content, options).map_err(|_| VaultError::Format)?;
    Ok(PrivateYaml(value))
}

fn object(value: &Value) -> Result<&Map<String, Value>, VaultError> {
    value.as_object().ok_or(VaultError::Format)
}

fn fields(map: &Map<String, Value>, allowed: &[&str]) -> Result<(), VaultError> {
    if map.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(VaultError::Format);
    }
    Ok(())
}

fn text(value: &Value) -> Result<&str, VaultError> {
    let text = value.as_str().ok_or(VaultError::Format)?;
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(VaultError::Format);
    }
    Ok(text)
}

fn public_text(value: &Value) -> Result<&str, VaultError> {
    public_str(text(value)?)
}

fn public_str(value: &str) -> Result<&str, VaultError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(VaultError::Format);
    }
    let lower = value.to_ascii_lowercase();
    if value.contains(super::SLOT_PREFIX)
        || super::contains_credentials(value)
        // Public YAML scalars can contain quotes and commas inside a URL. Inspect the
        // complete parsed scalar as well as the legacy INI-oriented text scanner.
        || Url::parse(value).is_ok_and(|parsed| url_has_credentials(&parsed))
        || [
            "ss://",
            "ssr://",
            "vmess://",
            "vless://",
            "trojan://",
            "hysteria://",
            "hysteria2://",
            "tuic://",
        ]
        .iter()
        .any(|scheme| lower.contains(scheme))
    {
        return Err(VaultError::Format);
    }
    Ok(value)
}

fn url_has_credentials(parsed: &Url) -> bool {
    !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query_pairs().any(|(name, _)| {
            super::credential_key(&name)
                || ["key", "auth", "access_token"]
                    .iter()
                    .any(|candidate| name.eq_ignore_ascii_case(candidate))
        })
}

fn strings(value: &Value) -> Result<(), VaultError> {
    for value in value.as_array().ok_or(VaultError::Format)? {
        public_text(value)?;
    }
    Ok(())
}

fn flag(value: &Value) -> Result<(), VaultError> {
    value.as_bool().map(|_| ()).ok_or(VaultError::Format)
}

fn integer(value: &Value) -> Result<(), VaultError> {
    value.as_u64().map(|_| ()).ok_or(VaultError::Format)
}

fn public_url(value: &Value) -> Result<(), VaultError> {
    let value = public_text(value)?;
    let parsed = Url::parse(value).map_err(|_| VaultError::Format)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || url_has_credentials(&parsed)
        || parsed.fragment().is_some()
    {
        return Err(VaultError::Format);
    }
    Ok(())
}

fn cache_path(value: &Value) -> Result<(), VaultError> {
    let value = public_text(value)?;
    if value.starts_with('/')
        || value.contains(['\\', ':'])
        || value.split('/').any(|part| part == "..")
    {
        return Err(VaultError::Format);
    }
    Ok(())
}

fn slot(value: &str) -> Option<String> {
    let digits = value.strip_prefix(super::SLOT_PREFIX)?.strip_suffix("}}")?;
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number: usize = digits.parse().ok()?;
    (1..=MAX_TEMPLATES)
        .contains(&number)
        .then(|| format!("SUBSCRIPTION_{number}"))
}

fn subscription_url(input: &str, https_only: bool) -> Result<(), VaultError> {
    // YAML serialization handles quotes and delimiters; line breaks and userinfo stay forbidden.
    if input.is_empty()
        || input.len() > MAX_SUBSCRIPTION_BYTES
        || input
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '\\' | '{' | '}'))
    {
        return Err(VaultError::Format);
    }
    let parsed = Url::parse(input).map_err(|_| VaultError::Format)?;
    if !(parsed.scheme() == "https" || !https_only && parsed.scheme() == "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(VaultError::Format);
    }
    Ok(())
}

fn health_check(value: &Value) -> Result<(), VaultError> {
    let map = object(value)?;
    fields(
        map,
        &[
            "enable",
            "url",
            "interval",
            "timeout",
            "lazy",
            "expected-status",
        ],
    )?;
    for (key, value) in map {
        match key.as_str() {
            "enable" | "lazy" => flag(value)?,
            "interval" | "timeout" => integer(value)?,
            "url" => public_url(value)?,
            _ => {
                public_text(value)?;
            }
        }
    }
    Ok(())
}

fn validate(value: &Value, mode: UrlMode) -> Result<(), VaultError> {
    let root = object(value)?;
    fields(
        root,
        &[
            "mode",
            "log-level",
            "ipv6",
            "allow-lan",
            "geodata-mode",
            "geox-url",
            "dns",
            "tun",
            "proxies",
            "proxy-providers",
            "proxy-groups",
            "rule-providers",
            "rules",
            "sub-rules",
        ],
    )?;
    for (key, value) in root {
        match key.as_str() {
            "ipv6" | "allow-lan" | "geodata-mode" => flag(value)?,
            "mode" | "log-level" => {
                public_text(value)?;
            }
            "proxies" if value.as_array().is_some_and(Vec::is_empty) => {}
            "proxies" => return Err(VaultError::Format),
            "geox-url" => {
                let map = object(value)?;
                fields(map, &["mmdb", "asn", "geoip", "geosite"])?;
                for value in map.values() {
                    public_url(value)?;
                }
            }
            "tun" => {
                let map = object(value)?;
                fields(
                    map,
                    &[
                        "enable",
                        "stack",
                        "auto-route",
                        "auto-detect-interface",
                        "strict-route",
                        "dns-hijack",
                        "mtu",
                    ],
                )?;
                for (key, value) in map {
                    match key.as_str() {
                        "stack" => {
                            if !matches!(value.as_str(), Some("system" | "gvisor" | "mixed")) {
                                return Err(VaultError::Format);
                            }
                        }
                        "dns-hijack" => {
                            strings(value)?;
                        }
                        "mtu" => integer(value)?,
                        _ => flag(value)?,
                    }
                }
            }
            "dns" => {
                let map = object(value)?;
                fields(
                    map,
                    &[
                        "enable",
                        "ipv6",
                        "enhanced-mode",
                        "use-hosts",
                        "use-system-hosts",
                        "respect-rules",
                        "default-nameserver",
                        "proxy-server-nameserver",
                        "nameserver",
                        "fallback",
                        "direct-nameserver",
                        "nameserver-policy",
                        "fake-ip-filter",
                        "fake-ip-range",
                    ],
                )?;
                for (key, value) in map {
                    match key.as_str() {
                        "enable" | "ipv6" | "use-hosts" | "use-system-hosts" | "respect-rules" => {
                            flag(value)?
                        }
                        "enhanced-mode" | "fake-ip-range" => {
                            public_text(value)?;
                        }
                        "nameserver-policy" => {
                            for (selector, upstreams) in object(value)? {
                                public_str(selector)?;
                                if upstreams.is_array() {
                                    strings(upstreams)?;
                                } else {
                                    public_text(upstreams)?;
                                }
                            }
                        }
                        _ => strings(value)?,
                    }
                }
            }
            _ => {}
        }
    }
    let providers = object(root.get("proxy-providers").ok_or(VaultError::Format)?)?;
    if providers.len() > MAX_TEMPLATES {
        return Err(VaultError::TooLarge);
    }
    for (name, value) in providers {
        public_str(name)?;
        let map = object(value)?;
        fields(
            map,
            &[
                "type",
                "url",
                "path",
                "interval",
                "proxy",
                "size-limit",
                "health-check",
                "filter",
                "exclude-filter",
                "exclude-type",
            ],
        )?;
        if map.get("type").and_then(Value::as_str) != Some("http") {
            return Err(VaultError::Format);
        }
        let url = text(map.get("url").ok_or(VaultError::Format)?)?;
        if slot(url).is_none() {
            if matches!(mode, UrlMode::Slots) {
                return Err(VaultError::Format);
            }
            subscription_url(url, false)?;
        }
        for (key, value) in map {
            match key.as_str() {
                "type" | "url" => {}
                "path" => cache_path(value)?,
                "interval" | "size-limit" => integer(value)?,
                "health-check" => health_check(value)?,
                _ => {
                    public_text(value)?;
                }
            }
        }
    }
    let groups = root
        .get("proxy-groups")
        .and_then(Value::as_array)
        .ok_or(VaultError::Format)?;
    if groups.is_empty() {
        return Err(VaultError::Format);
    }
    let mut names = Vec::new();
    for value in groups {
        let map = object(value)?;
        fields(
            map,
            &[
                "name",
                "type",
                "use",
                "proxies",
                "hidden",
                "icon",
                "url",
                "interval",
                "timeout",
                "tolerance",
                "lazy",
                "strategy",
                "filter",
                "exclude-filter",
                "exclude-type",
                "disable-udp",
                "expected-status",
                "max-failed-times",
                "empty-fallback",
                "default-selected",
                "include-all",
                "include-all-providers",
                "include-all-proxies",
            ],
        )?;
        let name = public_text(map.get("name").ok_or(VaultError::Format)?)?;
        if names.contains(&name) {
            return Err(VaultError::Format);
        }
        names.push(name);
        if !matches!(
            map.get("type").and_then(Value::as_str),
            Some("select" | "url-test" | "fallback" | "load-balance")
        ) {
            return Err(VaultError::Format);
        }
        for (key, value) in map {
            match key.as_str() {
                "use" | "proxies" => strings(value)?,
                "hidden"
                | "lazy"
                | "disable-udp"
                | "include-all"
                | "include-all-providers"
                | "include-all-proxies" => flag(value)?,
                "interval" | "timeout" | "tolerance" | "max-failed-times" => integer(value)?,
                "url" | "icon" => public_url(value)?,
                _ => {
                    public_text(value)?;
                }
            }
        }
    }
    let rules = root.get("rules").ok_or(VaultError::Format)?;
    strings(rules)?;
    if rules.as_array().is_none_or(Vec::is_empty) {
        return Err(VaultError::Format);
    }
    if let Some(value) = root.get("sub-rules") {
        for (name, rules) in object(value)? {
            public_str(name)?;
            strings(rules)?;
        }
    }
    if let Some(value) = root.get("rule-providers") {
        for (name, value) in object(value)? {
            public_str(name)?;
            let map = object(value)?;
            fields(
                map,
                &[
                    "type",
                    "behavior",
                    "format",
                    "url",
                    "path",
                    "interval",
                    "proxy",
                    "size-limit",
                    "payload",
                ],
            )?;
            match map.get("type").and_then(Value::as_str) {
                Some("http") if !map.contains_key("payload") => {
                    public_url(map.get("url").ok_or(VaultError::Format)?)?
                }
                Some("inline") if !map.contains_key("url") && !map.contains_key("path") => {
                    strings(map.get("payload").ok_or(VaultError::Format)?)?
                }
                _ => return Err(VaultError::Format),
            }
            for (key, value) in map {
                match key.as_str() {
                    "url" | "payload" => {}
                    "path" => cache_path(value)?,
                    "interval" | "size-limit" => integer(value)?,
                    _ => {
                        public_text(value)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn encrypted_resolver(value: &Value, bootstrap: bool) -> Result<(), VaultError> {
    let url = Url::parse(public_text(value)?).map_err(|_| VaultError::Format)?;
    if url.scheme() != "https"
        || url.host().is_none()
        || (bootstrap && !matches!(url.host(), Some(url::Host::Ipv4(_) | url::Host::Ipv6(_))))
        || !matches!(url.fragment(), None | Some("DIRECT"))
    {
        return Err(VaultError::Format);
    }
    Ok(())
}

fn guard_windows_dns(root: &mut Value) -> Result<(), VaultError> {
    let root = root.as_object_mut().ok_or(VaultError::Format)?;
    let dns = root
        .get_mut("dns")
        .and_then(Value::as_object_mut)
        .ok_or(VaultError::Format)?;
    for key in DNS_RESOLVERS {
        let required = matches!(*key, "nameserver" | "default-nameserver")
            || (*key == "proxy-server-nameserver"
                && dns.get("respect-rules") == Some(&Value::Bool(true)));
        let resolvers = dns
            .get(*key)
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        if required && resolvers.is_empty() {
            return Err(VaultError::Format);
        }
        for resolver in resolvers {
            encrypted_resolver(resolver, *key == "default-nameserver")?;
        }
    }
    let policies = dns
        .entry("nameserver-policy")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or(VaultError::Format)?;
    for upstreams in policies.values() {
        if let Some(upstreams) = upstreams.as_array() {
            if upstreams.is_empty() {
                return Err(VaultError::Format);
            }
            for upstream in upstreams {
                encrypted_resolver(upstream, false)?;
            }
        } else {
            encrypted_resolver(upstreams, false)?;
        }
    }
    policies
        .entry("geosite:cn")
        .or_insert_with(|| CN_DOH.into());
    dns.insert("enable".into(), true.into());

    let tun = root
        .entry("tun")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or(VaultError::Format)?;
    let hijack = tun
        .entry("dns-hijack")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or(VaultError::Format)?;
    if !hijack
        .iter()
        .any(|value| matches!(value.as_str(), Some("any:53" | "udp://any:53")))
    {
        hijack.push("any:53".into());
    }
    if !hijack
        .iter()
        .any(|value| value.as_str() == Some("tcp://any:53"))
    {
        hijack.push("tcp://any:53".into());
    }
    for flag in ["enable", "auto-route", "strict-route"] {
        tun.insert(flag.into(), true.into());
    }
    Ok(())
}

fn serialize(value: &Value) -> Result<Zeroizing<String>, VaultError> {
    let output = Zeroizing::new(serde_saphyr::to_string(value).map_err(|_| VaultError::Format)?);
    if output.len() > MAX_TEMPLATE_BYTES {
        return Err(VaultError::TooLarge);
    }
    Ok(output)
}

pub(super) fn is_config(content: &str) -> bool {
    parse(content).is_ok_and(|value| validate(&value.0, UrlMode::Import).is_ok())
}

fn contains_subscription(value: &Value, urls: &[&str]) -> bool {
    let matches = |text: &str| urls.iter().any(|url| text.contains(*url));
    match value {
        Value::String(text) => matches(text),
        Value::Array(values) => values
            .iter()
            .any(|value| contains_subscription(value, urls)),
        Value::Object(values) => values
            .iter()
            .any(|(key, value)| matches(key) || contains_subscription(value, urls)),
        _ => false,
    }
}

pub(super) fn sanitize(template: &Template) -> Result<Template, VaultError> {
    let mut value = parse(&template.content)?;
    validate(&value.0, UrlMode::Import)?;
    let providers = value
        .0
        .get_mut("proxy-providers")
        .and_then(Value::as_object_mut)
        .ok_or(VaultError::Format)?;
    let mut replaced = Vec::new();
    for (index, provider) in providers.values_mut().enumerate() {
        let Value::String(url) = provider.get_mut("url").ok_or(VaultError::Format)? else {
            return Err(VaultError::Format);
        };
        let original = Zeroizing::new(std::mem::replace(
            url,
            format!("{{{{SUBSCRIPTION_{}}}}}", index + 1),
        ));
        if slot(&original).is_none() {
            replaced.push(original);
        }
    }
    // An otherwise public-looking field can repeat a path-authenticated subscription.
    let originals = replaced.iter().map(|url| url.as_str()).collect::<Vec<_>>();
    if !originals.is_empty()
        && (originals.iter().any(|url| template.name.contains(*url))
            || contains_subscription(&value.0, &originals))
    {
        return Err(VaultError::Format);
    }
    let mut content = serialize(&value.0)?;
    Ok(Template {
        name: template.name.clone(),
        client: Client::FlClash,
        content: std::mem::take(&mut *content),
    })
}

pub(super) fn subscription_slots(template: &Template) -> Result<Vec<String>, VaultError> {
    let value = parse(&template.content)?;
    validate(&value.0, UrlMode::Slots)?;
    slots(&value.0)
}

fn slots(value: &Value) -> Result<Vec<String>, VaultError> {
    let mut names = Vec::new();
    for provider in object(value.get("proxy-providers").ok_or(VaultError::Format)?)?.values() {
        let name = slot(
            provider
                .get("url")
                .and_then(Value::as_str)
                .ok_or(VaultError::Format)?,
        )
        .ok_or(VaultError::Format)?;
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Ok(names)
}

pub(super) fn render(
    template: &Template,
    subscriptions: &BTreeMap<String, String>,
    windows_dns_guard: bool,
) -> Result<Zeroizing<String>, VaultError> {
    let mut value = parse(&template.content)?;
    validate(&value.0, UrlMode::Slots)?;
    if windows_dns_guard {
        guard_windows_dns(&mut value.0)?;
    }
    let names = slots(&value.0)?;
    if subscriptions.len() != names.len()
        || names.iter().any(|name| !subscriptions.contains_key(name))
    {
        return Err(VaultError::Format);
    }
    for url in subscriptions.values() {
        subscription_url(url, true)?;
    }
    let urls = subscriptions
        .values()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if !urls.is_empty() && contains_subscription(&value.0, &urls) {
        return Err(VaultError::Format);
    }
    for provider in value
        .0
        .get_mut("proxy-providers")
        .and_then(Value::as_object_mut)
        .ok_or(VaultError::Format)?
        .values_mut()
    {
        let map = provider.as_object_mut().ok_or(VaultError::Format)?;
        let name = slot(
            map.get("url")
                .and_then(Value::as_str)
                .ok_or(VaultError::Format)?,
        )
        .ok_or(VaultError::Format)?;
        map.insert(
            "url".to_owned(),
            subscriptions
                .get(&name)
                .ok_or(VaultError::Format)?
                .clone()
                .into(),
        );
    }
    serialize(&value.0)
}

#[cfg(test)]
mod tests;
