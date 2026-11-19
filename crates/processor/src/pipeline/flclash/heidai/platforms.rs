//! Render the same reviewed 217heidai domain suffixes for each client.

use crate::pipeline::mihomo_generator;
use std::fmt::Write as _;

const SURGE: &str = "ScriptHub.AdBlock.Heidai.Full.Surge.list";
const SHADOWROCKET: &str = "ScriptHub.AdBlock.Heidai.Full.Shadowrocket.module";
const MIHOMO: &str = "ScriptHub.AdBlock.Heidai.Full.Mihomo.yaml";
const SING_BOX: &str = "ScriptHub.AdBlock.Heidai.Full.SingBox.json";
const ADGUARD_HOME: &str = "ScriptHub.AdBlock.Heidai.Full.AdGuardHome.txt";

pub(super) fn render(domains: &[&str]) -> Result<Vec<(&'static str, String)>, String> {
    let mut surge = format!(
        "# Script Hub · 217heidai complete input · Surge RULE-SET\n# Total Rules: {}\n",
        domains.len()
    );
    let mut shadowrocket = String::from(
        "#!name=Script Hub · 217heidai Full Local\n\
         #!desc=Reviewed domain blocks from the complete 217heidai DNS release.\n\n\
         [Rule]\n",
    );
    let mut adguard_home = String::from(
        "! Script Hub · 217heidai complete input · AdGuard Home\n\
         ! This list includes the domain blocks after source exceptions and service protection.\n",
    );
    for host in domains {
        writeln!(surge, "DOMAIN-SUFFIX,{host}").expect("writing to a string cannot fail");
        writeln!(shadowrocket, "DOMAIN-SUFFIX,{host},REJECT")
            .expect("writing to a string cannot fail");
        writeln!(adguard_home, "||{host}^").expect("writing to a string cannot fail");
    }

    let mihomo = mihomo_generator::render_classical(&surge)?
        .ok_or("The approved 217heidai source contains no eligible domain rules")?;
    if mihomo.supported != domains.len() || mihomo.omitted != 0 || mihomo.adapted != 0 {
        return Err("Mihomo projection did not preserve all approved domain suffixes".into());
    }
    let sing_box = serde_json::to_string(&serde_json::json!({
        "version": 5,
        "rules": [{ "domain_suffix": domains }]
    }))
    .map_err(|error| error.to_string())?;

    Ok(vec![
        (SURGE, surge),
        (SHADOWROCKET, shadowrocket),
        (MIHOMO, mihomo.content),
        (SING_BOX, sing_box + "\n"),
        (ADGUARD_HOME, adguard_home),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn every_platform_preserves_the_same_suffix_matchers() {
        let hosts = ["ad.example.test", "xn--bcher-kva.example"];
        let outputs = render(&hosts)
            .unwrap()
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        assert_eq!(outputs.len(), 5);

        let expected_surge = hosts.map(|host| format!("DOMAIN-SUFFIX,{host}"));
        let expected_shadowrocket = hosts.map(|host| format!("DOMAIN-SUFFIX,{host},REJECT"));
        let expected_mihomo = hosts.map(|host| format!("  - 'DOMAIN-SUFFIX,{host}'"));
        let expected_adguard = hosts.map(|host| format!("||{host}^"));
        assert!(
            outputs[SURGE]
                .lines()
                .filter(|line| !line.starts_with('#'))
                .eq(expected_surge)
        );
        let (header, rules) = outputs[SHADOWROCKET].split_once("\n[Rule]\n").unwrap();
        assert!(
            header
                .lines()
                .all(|line| line.starts_with("#!") || line.is_empty())
        );
        assert!(rules.lines().eq(expected_shadowrocket));
        assert_eq!(outputs[MIHOMO].lines().next(), Some("payload:"));
        assert!(outputs[MIHOMO].lines().skip(1).eq(expected_mihomo));
        assert!(
            outputs[ADGUARD_HOME]
                .lines()
                .filter(|line| !line.starts_with('!'))
                .eq(expected_adguard)
        );
        let source: serde_json::Value = serde_json::from_str(&outputs[SING_BOX]).unwrap();
        assert_eq!(
            source,
            serde_json::json!({ "version": 5, "rules": [{ "domain_suffix": hosts }] })
        );
        assert!(render(&[]).is_err());
    }
}
