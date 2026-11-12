use std::fmt::Write;

const RAW_RULESETS: &str =
    "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets";
const RULESETS: [&str; 2] = ["Microsoft_direct", "Microsoft_proxy"];

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
    let entries = RULESETS.into_iter().zip([direct, proxy]);
    if matches!(client, "sing-box" | "singbox") {
        let rule_set = RULESETS.map(|tag| {
            serde_json::json!({
                "type": "remote", "tag": tag, "format": "binary",
                "url": format!("{RAW_RULESETS}/SingBox/{tag}_Singbox.srs"),
            })
        });
        let rules: Vec<_> = entries
            .map(|(tag, policy)| {
                serde_json::json!({"rule_set": [tag], "action": "route", "outbound": policy})
            })
            .collect();
        let route = serde_json::json!({"route": {"rule_set": rule_set, "rules": rules}});
        return serde_json::to_string_pretty(&route).map_err(|error| error.to_string());
    }

    let yaml = matches!(client, "mihomo" | "flclash");
    if !yaml && !matches!(client, "surge" | "shadowrocket") {
        return Err(
            "Unsupported client; choose surge, shadowrocket, flclash, mihomo or sing-box".into(),
        );
    }
    let mut output = if yaml {
        let mut output = String::from("rule-providers:\n");
        for name in RULESETS {
            writeln!(
                output,
                "  {name}:\n    type: http\n    behavior: classical\n    format: yaml\n    url: {RAW_RULESETS}/Mihomo/{name}.yaml\n    interval: 86400"
            )
            .map_err(|error| error.to_string())?;
        }
        output.push_str("rules:\n");
        output
    } else {
        "[Rule]\n".to_owned()
    };
    for (name, policy) in entries {
        if yaml {
            let row = serde_json::to_string(&format!("RULE-SET,{name},{policy}"))
                .map_err(|error| error.to_string())?;
            writeln!(output, "  - {row}").map_err(|error| error.to_string())?;
        } else {
            writeln!(
                output,
                "RULE-SET,{RAW_RULESETS}/RULE-SET/{name}.list,{policy}"
            )
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_clients_follow_the_live_split_with_direct_first() {
        for client in ["surge", "shadowrocket"] {
            let rendered = render(client, "My direct", "My proxy").unwrap();
            assert_eq!(
                rendered.lines().collect::<Vec<_>>(),
                [
                    "[Rule]",
                    &format!("RULE-SET,{RAW_RULESETS}/RULE-SET/Microsoft_direct.list,My direct"),
                    &format!("RULE-SET,{RAW_RULESETS}/RULE-SET/Microsoft_proxy.list,My proxy"),
                ]
            );
        }
        for client in ["flclash", "mihomo"] {
            let rendered = render(client, "My direct", "My proxy").unwrap();
            let (providers, rules) = rendered.split_once("\nrules:\n").unwrap();
            for name in RULESETS {
                assert!(providers.contains(&format!(
                    "  {name}:\n    type: http\n    behavior: classical\n    format: yaml\n    url: {RAW_RULESETS}/Mihomo/{name}.yaml"
                )));
            }
            assert_eq!(
                rules.lines().collect::<Vec<_>>(),
                [
                    r#"  - "RULE-SET,Microsoft_direct,My direct""#,
                    r#"  - "RULE-SET,Microsoft_proxy,My proxy""#,
                ]
            );
        }
        let output = render("sing-box", "direct", "proxy").unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        let route = &json["route"];
        let rule_set = route["rule_set"].as_array().unwrap();
        assert_eq!(rule_set.len(), RULESETS.len());
        for (provider, name) in rule_set.iter().zip(RULESETS) {
            assert_eq!(provider["tag"], name);
            assert_eq!(provider["type"], "remote");
            assert_eq!(provider["format"], "binary");
            assert_eq!(
                provider["url"],
                format!("{RAW_RULESETS}/SingBox/{name}_Singbox.srs")
            );
        }
        assert_eq!(
            route["rules"],
            serde_json::json!([
                {"rule_set": ["Microsoft_direct"], "action": "route", "outbound": "direct"},
                {"rule_set": ["Microsoft_proxy"], "action": "route", "outbound": "proxy"},
            ])
        );
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
