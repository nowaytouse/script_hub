use super::{VaultError, checked_push, inline_comment};
use zeroize::Zeroizing;

fn repository_ruleset(url: &str) -> bool {
    [
        "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/RULE-SET/",
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/RULE-SET/",
    ]
    .iter()
    .any(|prefix| {
        url.strip_prefix(prefix).is_some_and(|name| {
            matches!(
                name.strip_prefix("github/").unwrap_or(name),
                "Microsoft.list" | "GitHub.list"
            )
        })
    })
}

pub(super) fn upgrade(content: &str) -> Result<Option<Zeroizing<String>>, VaultError> {
    let mut output = Zeroizing::new(String::with_capacity(content.len()));
    let mut rule_section = false;
    let mut direct_position = 0;
    let mut microsoft = None;
    let mut github = None;
    for raw in content.split_inclusive('\n') {
        let (active, _) = inline_comment(raw.trim_end_matches(['\r', '\n']));
        let active = active.trim();
        if active.starts_with('[') && active.ends_with(']') {
            rule_section = active.eq_ignore_ascii_case("[Rule]");
            if rule_section {
                direct_position = output.len() + raw.len();
            }
        }
        if rule_section
            && let Some((kind, tail)) = active.split_once(',')
            && kind.eq_ignore_ascii_case("RULE-SET")
            && let Some((url, remainder)) = tail.split_once(',')
            && repository_ruleset(url.trim())
        {
            let url = url.trim();
            let policy = remainder.split(',').next().unwrap_or_default().trim();
            if let Some(base) = url.strip_suffix("Microsoft.list") {
                if microsoft.replace((base, policy)).is_some() {
                    return Err(VaultError::Format);
                }
                let end = raw.find(url).ok_or(VaultError::Format)? + url.len() - ".list".len();
                checked_push(&mut output, &raw[..end])?;
                checked_push(&mut output, "_proxy")?;
                checked_push(&mut output, &raw[end..])?;
            } else if github.replace(policy).is_some() {
                return Err(VaultError::Format);
            }
            continue;
        }
        checked_push(&mut output, raw)?;
    }
    let base = match (microsoft, github) {
        (None, None) => return Ok(None),
        (Some((base, proxy)), Some(github))
            if !proxy.is_empty() && !proxy.eq_ignore_ascii_case("DIRECT") && proxy == github =>
        {
            base
        }
        _ => return Err(VaultError::Format),
    };
    let ending = if output[..direct_position].ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let direct = format!("RULE-SET,{base}Microsoft_direct.list,DIRECT{ending}");
    if output.len().saturating_add(direct.len()) > super::MAX_TEMPLATE_BYTES {
        return Err(VaultError::TooLarge);
    }
    output.insert_str(direct_position, &direct);
    Ok(Some(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str =
        "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/RULE-SET/github/";

    #[test]
    fn upgrades_only_owned_rules_and_preserves_the_proxy_policy_and_other_sections() {
        let input = format!(
            "[Rule]\r\nDOMAIN,example.test,DIRECT\r\nRULE-SET,{BASE}GitHub.list,PROXY\r\nRULE-SET,{BASE}Microsoft.list,PROXY,no-resolve\r\nRULE-SET,https://other.test/GitHub.list,DIRECT\r\nFINAL,DIRECT\r\n[Other]\r\nRULE-SET,{BASE}GitHub.list,PROXY"
        );
        let output = upgrade(&input).unwrap().unwrap();
        assert_eq!(
            output.as_str(),
            format!(
                "[Rule]\r\nRULE-SET,{BASE}Microsoft_direct.list,DIRECT\r\nDOMAIN,example.test,DIRECT\r\nRULE-SET,{BASE}Microsoft_proxy.list,PROXY,no-resolve\r\nRULE-SET,https://other.test/GitHub.list,DIRECT\r\nFINAL,DIRECT\r\n[Other]\r\nRULE-SET,{BASE}GitHub.list,PROXY"
            )
        );
        assert!(upgrade(&output).unwrap().is_none());
    }

    #[test]
    fn rejects_ambiguous_legacy_proxy_policies() {
        let input = format!(
            "[Rule]\nRULE-SET,{BASE}GitHub.list,Other proxy\nRULE-SET,{BASE}Microsoft.list,PROXY\nFINAL,DIRECT"
        );
        assert!(matches!(upgrade(&input), Err(VaultError::Format)));
    }
}
