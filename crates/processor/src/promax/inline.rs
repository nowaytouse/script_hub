use super::rule::{RuleKind, SurgeRule, split_fields};
use std::collections::BTreeMap;

const PREFIX: &str = "Script Hub PROMAX ";
const REQUIREMENT: &str = "#!requirement=CORE_VERSION>=6010000";
const MAX_GROUP_ROWS: usize = 4096;

fn is_ip(kind: RuleKind) -> bool {
    matches!(
        kind,
        RuleKind::IpCidr | RuleKind::IpCidr6 | RuleKind::IpAsn | RuleKind::GeoIp
    )
}

fn is_port(kind: RuleKind) -> bool {
    matches!(
        kind,
        RuleKind::DestPort | RuleKind::SrcPort | RuleKind::InPort
    )
}

fn is_reject(policy: &str) -> bool {
    matches!(policy, "REJECT" | "REJECT-DROP" | "REJECT-NO-DROP")
}

fn grouped_rule(line: &str, in_ports: bool) -> Option<(SurgeRule, &'static str)> {
    // Only equivalent port rows and established pre-matching IP rows move here.
    if !in_ports && !line.ends_with(",no-resolve,pre-matching") {
        return None;
    }
    let rule = SurgeRule::parse(line).ok()?;
    if !rule.policy.as_deref().is_some_and(is_reject) {
        return None;
    }
    if in_ports && is_port(rule.kind) && rule.options.is_empty() {
        Some((rule, "Firewall ports"))
    } else if is_ip(rule.kind) && rule.options == ["NO-RESOLVE", "PRE-MATCHING"] {
        Some((rule, "IP pre-match"))
    } else {
        None
    }
}

pub(super) fn organize(content: &str) -> String {
    let mut output = Vec::new();
    let mut groups = Vec::new();
    let mut current = None;
    let mut in_rule = false;
    let mut in_ports = false;
    for line in content.lines() {
        if line.starts_with('[') && line.ends_with(']') {
            in_rule = line == "[Rule]";
        }
        match line {
            "# --- SYNCED PORT RULES START ---" => in_ports = true,
            "# --- SYNCED PORT RULES END ---" => in_ports = false,
            _ => {}
        }
        if let Some((rule, label)) = in_rule.then(|| grouped_rule(line, in_ports)).flatten() {
            let policy = rule
                .policy
                .as_deref()
                .expect("grouped rule requires a policy");
            let key = (label, policy.to_owned());
            if current.as_ref() != Some(&key)
                || groups
                    .last()
                    .is_some_and(|(_, rows): &(String, Vec<String>)| rows.len() == MAX_GROUP_ROWS)
            {
                let name = format!("{PREFIX}{label} {:02}", groups.len() + 1);
                let pre = if is_ip(rule.kind) {
                    ",pre-matching"
                } else {
                    ""
                };
                output.push(format!("RULE-SET,{name},{policy},no-resolve{pre}"));
                groups.push((name, Vec::new()));
                current = Some(key);
            }
            groups
                .last_mut()
                .expect("current group exists")
                .1
                .push(rule.render_external());
        } else {
            current = None;
            output.push(line.to_owned());
        }
    }
    if groups.is_empty() {
        return content.to_owned();
    }
    if !output.iter().any(|line| line == REQUIREMENT) {
        output.insert(1, REQUIREMENT.to_owned());
    }
    for (name, rows) in groups {
        output.push(String::new());
        output.push(format!("[Ruleset {name}]"));
        output.extend(rows);
    }
    output.join("\n") + "\n"
}

/// Expand only our bounded, non-nested format; arbitrary profiles are untouched.
pub(super) fn expand(content: &str) -> Result<Option<String>, &'static str> {
    let Some(_) = content
        .lines()
        .find(|line| line.starts_with(&format!("[Ruleset {PREFIX}")))
    else {
        return Ok(None);
    };
    if !content.lines().any(|line| line == REQUIREMENT) {
        return Err("PROMAX inline rulesets require CORE_VERSION>=6010000");
    }

    let mut groups = BTreeMap::new();
    let mut visible = Vec::new();
    let mut current = None;
    for line in content.lines() {
        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            current = section
                .strip_prefix("Ruleset ")
                .filter(|name| name.starts_with(PREFIX));
            if let Some(name) = current {
                if groups.insert(name, Vec::new()).is_some() {
                    return Err("duplicate PROMAX inline ruleset");
                }
                if visible.last() == Some(&"") {
                    visible.pop();
                }
                continue;
            }
        }
        if let Some(name) = current {
            if let Some(row) = super::validation::external_rule_line(line) {
                let rows = groups.get_mut(name).expect("current group exists");
                if rows.len() >= MAX_GROUP_ROWS {
                    return Err("PROMAX inline ruleset exceeds its row limit");
                }
                rows.push(row);
            }
        } else {
            visible.push(line);
        }
    }

    let mut in_rule = false;
    let mut output = String::with_capacity(content.len());
    for line in visible {
        if line.starts_with('[') && line.ends_with(']') {
            in_rule = line == "[Rule]";
        }
        if in_rule && line.starts_with("RULE-SET,Script Hub PROMAX ") {
            let fields =
                split_fields(line).map_err(|_| "invalid PROMAX inline ruleset reference")?;
            let (name, policy, pre) = match fields.as_slice() {
                [_, name, policy, no] if no == "no-resolve" => (name, policy, false),
                [_, name, policy, no, pre] if no == "no-resolve" && pre == "pre-matching" => {
                    (name, policy, true)
                }
                _ => return Err("invalid PROMAX inline ruleset options"),
            };
            if !is_reject(policy) {
                return Err("invalid PROMAX inline ruleset policy");
            }
            let rows = groups
                .remove(name.as_str())
                .ok_or("missing or repeated PROMAX inline ruleset reference")?;
            if rows.is_empty() {
                return Err("empty PROMAX inline ruleset");
            }
            for row in rows {
                let mut rule = SurgeRule::parse(row).map_err(|_| "invalid PROMAX inline rule")?;
                if rule.policy.is_some()
                    || if pre {
                        !is_ip(rule.kind) || rule.options != ["NO-RESOLVE"]
                    } else {
                        !is_port(rule.kind) || !rule.options.is_empty()
                    }
                {
                    return Err("invalid matcher or options inside PROMAX inline ruleset");
                }
                if pre {
                    rule.options.push("PRE-MATCHING".into());
                }
                output.push_str(&rule.render_module(policy));
                output.push('\n');
            }
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if !groups.is_empty() {
        return Err("unreferenced PROMAX inline ruleset");
    }
    Ok(Some(output.trim_end().to_owned() + "\n"))
}

#[cfg(test)]
mod tests {
    use super::{expand, organize};
    use crate::promax::{
        artifact::convert_surge_to_shadowrocket,
        validation::{validate_shadowrocket_module, validate_surge_module},
    };

    const FIXTURE: &str = "\
#!name=PROMAX fixture
#!requirement=CORE_VERSION>=6010000
[IP Rewrite]
192.0.2.10 = reject
[Rule]
# --- SYNCED PORT RULES START ---
DEST-PORT,4444,REJECT-DROP
DEST-PORT,5555,REJECT
DEST-PORT,6666,REJECT
# --- SYNCED PORT RULES END ---
IP-CIDR,192.0.2.10/32,REJECT,no-resolve,pre-matching
IP-ASN,64496,REJECT,no-resolve,pre-matching
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Local.list,REJECT,no-resolve
IP-CIDR,192.0.2.10/32,REJECT-DROP,no-resolve
";

    #[test]
    fn groups_round_trip_without_moving_filters_or_changing_policy() {
        let module = organize(FIXTURE);
        assert!(module.contains("[Ruleset Script Hub PROMAX Firewall ports 01]\nDEST-PORT,4444"));
        assert!(module.contains("[Ruleset Script Hub PROMAX Firewall ports 02]\nDEST-PORT,5555"));
        assert!(module.contains(
            "[Ruleset Script Hub PROMAX IP pre-match 03]\nIP-CIDR,192.0.2.10/32,no-resolve"
        ));
        assert_eq!(expand(&module).unwrap().unwrap(), FIXTURE);
        assert_eq!(organize(&module), module);
        assert!(validate_surge_module(&module).is_empty());
        assert!(!validate_shadowrocket_module(&module).is_empty());
        assert_eq!(
            convert_surge_to_shadowrocket(&module),
            convert_surge_to_shadowrocket(FIXTURE)
        );
    }

    #[test]
    fn rejects_invalid_groups_and_keeps_safety_checks_on_grouped_rows() {
        let module = organize(FIXTURE);
        for invalid in [
            module.replace(super::REQUIREMENT, ""),
            module.replace("IP-ASN,64496,no-resolve", "IP-ASN,64496"),
            module.replace(
                "[Ruleset Script Hub PROMAX Firewall ports 01]",
                "[Ruleset Script Hub PROMAX Firewall ports 02]",
            ),
            module.replace("DEST-PORT,4444\n", "DEST-PORT,5223\n"),
            module.replace("DEST-PORT,4444\n", "DEST-PORT,4444,REJECT\n"),
        ] {
            assert!(!validate_surge_module(&invalid).is_empty());
        }
    }
}
