use std::fmt::Write;

use super::general_update::MICROSOFT_SIGN_IN_HOSTS;

const GITHUB: &str = include_str!("../../../../rulesets/Sources/custom/GitHub_manual.list");

pub fn render(client: &str, direct: &str, proxy: &str) -> Result<String, String> {
    if [direct, proxy].into_iter().any(|label| {
        label.is_empty()
            || label.len() > 128
            || label.trim() != label
            || label
                .chars()
                .any(|c| c.is_control() || matches!(c, ',' | '#' | ';' | '[' | ']' | '='))
    }) || direct == proxy
    {
        return Err("Specify two distinct existing policy names without rule delimiters".into());
    }
    let github = GITHUB
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            line.split_once(',')
                .filter(|(kind, host)| {
                    matches!(*kind, "DOMAIN" | "DOMAIN-SUFFIX")
                        && !host.contains(',')
                        && crate::normalize_rule(line).as_deref() == Some(line)
                })
                .ok_or("Invalid curated GitHub routing row")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if matches!(client, "sing-box" | "singbox") {
        let (domains, suffixes): (Vec<_>, Vec<_>) =
            github.into_iter().partition(|(kind, _)| *kind == "DOMAIN");
        let route = serde_json::json!({"route": {"rules": [
            {"domain": MICROSOFT_SIGN_IN_HOSTS, "action": "route", "outbound": direct},
            {"domain": domains.into_iter().map(|(_, host)| host).collect::<Vec<_>>(),
             "action": "route", "outbound": proxy},
            {"domain_suffix": suffixes.into_iter().map(|(_, host)| host).collect::<Vec<_>>(),
             "action": "route", "outbound": proxy},
        ]}});
        return serde_json::to_string_pretty(&route).map_err(|error| error.to_string());
    }

    let yaml = matches!(client, "mihomo" | "flclash");
    if !yaml && !matches!(client, "surge" | "shadowrocket") {
        return Err(
            "Unsupported client; choose surge, shadowrocket, flclash, mihomo or sing-box".into(),
        );
    }
    let mut output = if yaml {
        "rules:\n".to_owned()
    } else {
        "[Rule]\n".to_owned()
    };
    let sign_in = MICROSOFT_SIGN_IN_HOSTS
        .iter()
        .map(|host| ("DOMAIN", *host, direct));
    for (kind, host, policy) in
        sign_in.chain(github.into_iter().map(|(kind, host)| (kind, host, proxy)))
    {
        if yaml {
            let row = serde_json::to_string(&format!("{kind},{host},{policy}"))
                .map_err(|error| error.to_string())?;
            writeln!(output, "  - {row}").map_err(|error| error.to_string())?;
        } else {
            writeln!(output, "{kind},{host},{policy}").map_err(|error| error.to_string())?;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_clients_preserve_exact_sign_in_and_github_suffix_ownership() {
        for client in ["surge", "shadowrocket", "flclash", "mihomo"] {
            let rendered = render(client, "DIRECT", "GitHub proxy").unwrap();
            let sign_in = rendered
                .find("DOMAIN,login.microsoftonline.com,DIRECT")
                .unwrap();
            let github = rendered
                .find("DOMAIN-SUFFIX,github.com,GitHub proxy")
                .unwrap();
            assert!(sign_in < github);
            assert!(rendered.contains("DOMAIN,github-cloud.s3.amazonaws.com,GitHub proxy"));
            assert!(!rendered.contains("DOMAIN-SUFFIX,live.com"));
            assert!(!rendered.contains("DOMAIN-SUFFIX,s3.amazonaws.com"));
            assert!(!rendered.contains("FINAL,") && !rendered.contains("MATCH,"));
        }
        let output = render("sing-box", "direct", "proxy").unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        let route = &json["route"];
        let rules = route["rules"].as_array().unwrap();
        assert_eq!(
            rules[0]["domain"],
            serde_json::json!(MICROSOFT_SIGN_IN_HOSTS)
        );
        assert_eq!(rules[0]["outbound"], "direct");
        assert!(
            rules[2]["domain_suffix"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("github.com"))
        );
        assert_eq!(rules[2]["outbound"], "proxy");
        assert!(route.get("final").is_none());
    }

    #[test]
    fn rejects_injected_rows_and_unknown_clients() {
        for client in ["surge", "mihomo", "sing-box"] {
            for bad in ["", "same", "a\nFINAL,DIRECT", "a,REJECT", "a#note"] {
                assert!(render(client, "same", bad).is_err());
            }
        }
        assert!(render("other", "DIRECT", "Proxy").is_err());
    }
}
