//! Subscription slots and conservative sanitization for encrypted public templates.

use super::{Client, MAX_TEMPLATE_BYTES, MAX_TEMPLATES, Template, VaultError};
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;
use url::Url;
use zeroize::Zeroizing;

const SLOT_PREFIX: &str = "{{SUBSCRIPTION_";
const MAX_SUBSCRIPTION_BYTES: usize = 8192;
static URLS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)https?://[^\s,<>"']+"#).expect("constant URL pattern"));
static FIELD_ASSIGNMENTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:^|[,\s{?&])\\?["']?([a-z][a-z0-9_-]*)\\?["']?\s*[:=]"#)
        .expect("constant field assignment pattern")
});

fn checked_push(output: &mut String, value: &str) -> Result<(), VaultError> {
    if output.len().saturating_add(value.len()) > MAX_TEMPLATE_BYTES {
        return Err(VaultError::TooLarge);
    }
    output.push_str(value);
    Ok(())
}

fn slot_ranges(content: &str) -> Result<Vec<(usize, usize, String)>, VaultError> {
    let mut slots = Vec::new();
    let mut offset = 0;
    while let Some(relative) = content[offset..].find(SLOT_PREFIX) {
        let start = offset + relative;
        let number_start = start + SLOT_PREFIX.len();
        let tail = &content[number_start..];
        let digits_len = tail.find("}}").ok_or(VaultError::Format)?;
        let digits = &tail[..digits_len];
        if digits.starts_with('0') || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(VaultError::Format);
        }
        let number: usize = digits.parse().map_err(|_| VaultError::Format)?;
        if !(1..=MAX_TEMPLATES).contains(&number) {
            return Err(VaultError::Format);
        }
        let end = number_start + digits_len + 2;
        slots.push((start, end, format!("SUBSCRIPTION_{number}")));
        offset = end;
    }
    Ok(slots)
}

fn credential_key(key: &str) -> bool {
    let key = key.trim();
    key.get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http-api"))
        || [
            "ca-p12",
            "ca-passphrase",
            "passphrase",
            "password",
            "passwd",
            "token",
            "secret",
            "api-key",
            "api_key",
            "private-key",
            "private_key",
            "preshared-key",
            "preshared_key",
            "authorization",
            "external-controller",
            "external-controller-tls",
        ]
        .iter()
        .any(|candidate| key.eq_ignore_ascii_case(candidate))
}

fn contains_credentials(content: &str) -> bool {
    FIELD_ASSIGNMENTS.captures_iter(content).any(|capture| {
        capture
            .get(1)
            .is_some_and(|key| credential_key(key.as_str()))
    }) || URLS.find_iter(content).any(|matched| {
        let Ok(parsed) = Url::parse(matched.as_str()) else {
            return true;
        };
        !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query_pairs().any(|(name, _)| {
                credential_key(&name)
                    || ["key", "auth", "access_token"]
                        .iter()
                        .any(|candidate| name.eq_ignore_ascii_case(candidate))
            })
    })
}

fn inline_comment(line: &str) -> (&str, &str) {
    let boundary = line.char_indices().find_map(|(index, character)| {
        if !matches!(character, ' ' | '\t') {
            return None;
        }
        let following = line[index..].trim_start_matches([' ', '\t']);
        (following.starts_with('#') || following.starts_with("//")).then_some(index)
    });
    boundary.map_or((line, ""), |index| (&line[..index], &line[index..]))
}

fn policy_paths(line: &str, counter: &mut usize) -> Result<Zeroizing<String>, VaultError> {
    let mut output = Zeroizing::new(String::new());
    let mut found = false;
    for segment in line.split_inclusive(',') {
        let (field, comma) = segment
            .strip_suffix(',')
            .map_or((segment, ""), |field| (field, ","));
        let Some((key, value)) = field.split_once('=') else {
            checked_push(&mut output, segment)?;
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("policy-path") {
            checked_push(&mut output, segment)?;
            continue;
        }
        if found {
            return Err(VaultError::Format);
        }
        found = true;
        // Original values are discarded; validate user-supplied URLs when rendering.
        *counter += 1;
        if *counter > MAX_TEMPLATES {
            return Err(VaultError::TooLarge);
        }
        let prefix_len = field.len() - value.len();
        checked_push(&mut output, &field[..prefix_len])?;
        checked_push(
            &mut output,
            &value[..value.len() - value.trim_start().len()],
        )?;
        checked_push(&mut output, &format!("{{{{SUBSCRIPTION_{counter}}}}}"))?;
        if !value.trim().is_empty() {
            checked_push(&mut output, &value[value.trim_end().len()..])?;
        }
        checked_push(&mut output, comma)?;
    }
    Ok(output)
}

/// Removes supported private fields before a template is placed in a public vault.
pub fn sanitize(template: &Template) -> Result<Template, VaultError> {
    if template.client == Client::Other || template.content.contains('\0') {
        return Err(VaultError::Format);
    }
    if template.content.len() > MAX_TEMPLATE_BYTES {
        return Err(VaultError::TooLarge);
    }
    let mut output = Zeroizing::new(String::new());
    let mut section = "";
    let mut counter = 0;
    for raw in template.content.split_inclusive('\n') {
        let (line, ending) = if let Some(line) = raw.strip_suffix("\r\n") {
            (line, "\r\n")
        } else if let Some(line) = raw.strip_suffix('\n') {
            (line, "\n")
        } else {
            (raw, "")
        };
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed.starts_with(['#', ';']) || trimmed.starts_with("//") {
            if !URLS.is_match(line) && !contains_credentials(line) {
                checked_push(&mut output, raw)?;
            }
            continue;
        }
        if let Some(header) = trimmed
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            section = header;
            checked_push(&mut output, raw)?;
            continue;
        }
        let (active, comment) = inline_comment(line);
        let policy_group = section.eq_ignore_ascii_case("Proxy Group");
        if !policy_group
            && active
                .split_once('=')
                .is_some_and(|(key, _)| credential_key(key))
        {
            continue;
        }
        if section.eq_ignore_ascii_case("Proxy") && !active.trim().is_empty() {
            return Err(VaultError::Format);
        }
        let replaced = if template.client == Client::Surge && policy_group {
            Some(policy_paths(active, &mut counter)?)
        } else {
            if active.split(',').any(|field| {
                field
                    .split_once('=')
                    .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("policy-path"))
            }) {
                return Err(VaultError::Format);
            }
            None
        };
        let active = replaced.as_ref().map_or(active, |value| value.as_str());
        let parameters = if policy_group {
            active.split_once('=').map_or(active, |(_, value)| value)
        } else {
            active
        };
        if contains_credentials(parameters) {
            return Err(VaultError::Format);
        }
        checked_push(&mut output, active)?;
        if !URLS.is_match(comment) && !contains_credentials(comment) {
            checked_push(&mut output, comment)?;
        }
        checked_push(&mut output, ending)?;
    }
    Ok(Template {
        name: template.name.clone(),
        client: template.client,
        content: std::mem::take(&mut *output),
    })
}

/// Returns the unique subscription slot names in their first appearance order.
pub fn subscription_slots(template: &Template) -> Vec<String> {
    let Ok(ranges) = slot_ranges(&template.content) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for (_, _, name) in ranges {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Renders only explicit subscription slots, without fetching their URLs.
pub fn render(
    template: &Template,
    subscriptions: &BTreeMap<String, String>,
) -> Result<Zeroizing<String>, VaultError> {
    if template.content.len() > MAX_TEMPLATE_BYTES || template.content.contains('\0') {
        return Err(VaultError::TooLarge);
    }
    let ranges = slot_ranges(&template.content)?;
    let names = subscription_slots(template);
    if subscriptions.len() != names.len()
        || names.iter().any(|name| !subscriptions.contains_key(name))
    {
        return Err(VaultError::Format);
    }
    for value in subscriptions.values() {
        validate_subscription_url(value)?;
    }
    let mut output = Zeroizing::new(String::new());
    let mut offset = 0;
    for (start, end, name) in ranges {
        checked_push(&mut output, &template.content[offset..start])?;
        checked_push(
            &mut output,
            subscriptions.get(&name).ok_or(VaultError::Format)?,
        )?;
        offset = end;
    }
    checked_push(&mut output, &template.content[offset..])?;
    Ok(output)
}

/// Validates one HTTPS subscription URL for insertion into a configuration field.
pub fn validate_subscription_url(input: &str) -> Result<(), VaultError> {
    if input.is_empty()
        || input.len() > MAX_SUBSCRIPTION_BYTES
        || input.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, ',' | '#' | '\'' | '"' | '\\' | '{' | '}')
        })
    {
        return Err(VaultError::Format);
    }
    let parsed = Url::parse(input).map_err(|_| VaultError::Format)?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(VaultError::Format);
    }
    Ok(())
}

/// Imports a UTF-8 configuration without normalizing its bytes or logging its path.
pub fn import_template(path: &Path, client: Client) -> Result<Template, VaultError> {
    let bytes = Zeroizing::new(super::read_bounded(path)?);
    if bytes.len() > MAX_TEMPLATE_BYTES {
        return Err(VaultError::TooLarge);
    }
    let content = std::str::from_utf8(&bytes).map_err(|_| VaultError::Format)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(VaultError::Format)?;
    if content.is_empty()
        || content.contains('\0')
        || name.len() > 256
        || name.contains(['\r', '\n'])
    {
        return Err(VaultError::Format);
    }
    Ok(Template {
        name: name.to_string(),
        client,
        content: content.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(client: Client, content: &str) -> Template {
        Template {
            name: "fixture.conf".to_string(),
            client,
            content: content.to_string(),
        }
    }

    #[test]
    fn public_surge_template_replaces_only_policy_subscriptions() {
        let original = template(
            Client::Surge,
            "[Proxy Group]\r\nOne = select, policy-path=https://private.fixture/sub/one?token=secret, update-interval=3600\r\nTwo = url-test, policy-path=https://private.fixture/sub/two // keep\r\n[Rule]\r\nRULE-SET,https://rules.fixture/public.list,Example\r\n",
        );
        let sanitized = sanitize(&original).unwrap();
        assert_eq!(
            sanitized.content,
            "[Proxy Group]\r\nOne = select, policy-path={{SUBSCRIPTION_1}}, update-interval=3600\r\nTwo = url-test, policy-path={{SUBSCRIPTION_2}} // keep\r\n[Rule]\r\nRULE-SET,https://rules.fixture/public.list,Example\r\n"
        );
        assert_eq!(
            subscription_slots(&sanitized),
            ["SUBSCRIPTION_1", "SUBSCRIPTION_2"]
        );
        assert!(original.content.contains("token=secret"));
        assert_eq!(sanitize(&sanitized).unwrap().content, sanitized.content);
    }

    #[test]
    fn public_templates_remove_credentials_and_private_comment_urls() {
        let original = template(
            Client::Surge,
            "[General]\nhttp-api = local-secret@127.0.0.1:6171\n# Old subscription https://private.fixture/opaque-secret\n# ordinary guidance\n[MITM]\nca-p12 = private-certificate\nca-passphrase = private-passphrase\nhostname = example.test\n",
        );
        assert_eq!(
            sanitize(&original).unwrap().content,
            "[General]\n# ordinary guidance\n[MITM]\nhostname = example.test\n"
        );
    }

    #[test]
    fn unsupported_embedded_credentials_fail_without_disclosing_them() {
        for content in [
            "[Proxy]\nExample = trojan, private.fixture, 443, password=hidden-value\n",
            "[Script]\nexample = http-request, script-path=https://user:password@fixture.test/script.js\n",
            "[Script]\nexample = http-response, script-path=https://fixture.test/script.js?to%6ben=hidden-value\n",
            "[General]\nencrypted-dns-server = https://[::1]/dns?to%6ben=hidden-value\n",
            "[Script]\nexample = http-request, script-path=https://fixture.test/script.js?KEY=hidden-value\n",
            "[Script]\nexample = http-response, argument={\"secret\":\"hidden-value\"}\n",
        ] {
            let result = sanitize(&template(Client::Surge, content));
            assert!(result.is_err());
            assert!(!result.err().unwrap().to_string().contains("hidden-value"));
        }
    }

    #[test]
    fn shadowrocket_configuration_does_not_gain_surge_subscription_fields() {
        let original = template(
            Client::Shadowrocket,
            "[General]\ndns-server = system\n[Rule]\nDOMAIN,example.test,Example\n",
        );
        let sanitized = sanitize(&original).unwrap();
        assert_eq!(sanitized.content, original.content);
        assert!(subscription_slots(&sanitized).is_empty());
        assert_eq!(
            &*render(&sanitized, &BTreeMap::new()).unwrap(),
            &original.content
        );
    }

    #[test]
    fn rendering_requires_each_explicit_binding_and_preserves_other_urls() {
        let configured = template(
            Client::Surge,
            "[Proxy Group]\nExample = select, policy-path={{SUBSCRIPTION_1}}\n[Rule]\nRULE-SET,https://rules.fixture/public.list,Example\n",
        );
        assert!(render(&configured, &BTreeMap::new()).is_err());
        let bindings = BTreeMap::from([(
            "SUBSCRIPTION_1".to_string(),
            "https://subscription.fixture/path?token=new-value".to_string(),
        )]);
        assert_eq!(
            &*render(&configured, &bindings).unwrap(),
            "[Proxy Group]\nExample = select, policy-path=https://subscription.fixture/path?token=new-value\n[Rule]\nRULE-SET,https://rules.fixture/public.list,Example\n"
        );
        let mut extra = bindings;
        extra.insert(
            "SUBSCRIPTION_2".to_string(),
            "https://fixture.test/unused".to_string(),
        );
        assert!(render(&configured, &extra).is_err());
    }

    #[test]
    fn subscription_validation_rejects_configuration_injection_and_userinfo() {
        for value in [
            "http://fixture.test/sub",
            "https://fixture.test/sub\n[Rule]",
            "https://fixture.test/sub,REJECT",
            "https://fixture.test/sub#fragment",
            "https://fixture.test/sub\"",
            "https://fixture.test/sub'",
            "https://user:password@fixture.test/sub",
            "https://fixture.test\\sub",
            "https://fixture.test:99999/sub",
            "https://",
        ] {
            assert!(validate_subscription_url(value).is_err());
        }
        for value in [
            "https://fixture.test",
            "https://fixture.test/sub?token=value&format=surge",
            "https://[::1]:8443/sub",
        ] {
            assert!(validate_subscription_url(value).is_ok());
        }
    }

    #[test]
    fn malformed_or_out_of_range_slots_never_produce_an_export() {
        for value in [
            "{{SUBSCRIPTION_0}}",
            "{{SUBSCRIPTION_65}}",
            "{{SUBSCRIPTION_1}",
            "{{SUBSCRIPTION_01}}",
        ] {
            assert!(render(&template(Client::Surge, value), &BTreeMap::new()).is_err());
        }
    }

    #[test]
    fn public_templates_preserve_ipv6_dns_urls() {
        let original = template(
            Client::Surge,
            "[General]\nencrypted-dns-server = https://[2606:4700:4700::1111]/dns-query\n",
        );
        assert_eq!(sanitize(&original).unwrap().content, original.content);
    }

    #[test]
    fn a_policy_group_name_is_not_treated_as_a_credential_directive() {
        let original = template(
            Client::Surge,
            "[Proxy Group]\nToken = select, policy-path=https://subscription.fixture/list\n",
        );
        assert_eq!(
            sanitize(&original).unwrap().content,
            "[Proxy Group]\nToken = select, policy-path={{SUBSCRIPTION_1}}\n"
        );
    }

    #[test]
    fn public_template_preparation_replaces_unconfigured_subscription_values() {
        for value in [
            "",
            "YOUR_SUBSCRIPTION_URL",
            "填写订阅链接",
            "http://old.fixture/sub",
        ] {
            let original = template(
                Client::Surge,
                &format!(
                    "[Proxy Group]\nExample = select, policy-path={value}, update-interval=3600\n"
                ),
            );
            let prepared = sanitize(&original).unwrap();
            assert_eq!(
                prepared.content,
                "[Proxy Group]\nExample = select, policy-path={{SUBSCRIPTION_1}}, update-interval=3600\n"
            );
            assert!(render(&prepared, &BTreeMap::new()).is_err());
        }
    }

    #[test]
    fn large_slot_expansion_is_rejected_before_producing_a_large_export() {
        let original = template(Client::Surge, &"{{SUBSCRIPTION_1}}".repeat(200));
        let binding = BTreeMap::from([(
            "SUBSCRIPTION_1".to_string(),
            format!("https://subscription.fixture/{}", "x".repeat(8000)),
        )]);
        assert!(matches!(
            render(&original, &binding),
            Err(VaultError::TooLarge)
        ));
    }

    #[test]
    fn configuration_import_preserves_utf8_bom_and_crlf() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "script-hub-template-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("模板.conf");
        let contents = "\u{feff}[General]\r\ndns-server = system\r\n";
        std::fs::write(&path, contents).unwrap();
        let imported = import_template(&path, Client::Shadowrocket).unwrap();
        assert_eq!(imported.name, "模板.conf");
        assert_eq!(imported.content.as_bytes(), contents.as_bytes());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
#[test]
fn public_templates_reject_embedded_certificate_and_passphrase_aliases() {
    for key in [
        "passphrase",
        "ca-passphrase",
        "ca-p12",
        "http-api-key",
        "external-controller",
    ] {
        let input = Template {
            name: "synthetic fixture".to_owned(),
            client: Client::Surge,
            content: format!(
                "[Script]\nfixture = http-request, script-path=https://example.test/a.js, argument={{\"{key}\":\"synthetic-private-value\"}}\n"
            ),
        };
        assert!(
            matches!(sanitize(&input), Err(VaultError::Format)),
            "embedded {key} was not rejected"
        );
    }
}
