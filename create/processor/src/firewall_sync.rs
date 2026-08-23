use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

static PORT_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IN-PORT|DEST-PORT|SRC-PORT)").unwrap());

static VERSION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"#!version=.*").unwrap());

// Never turn normal Apple platform, LAN discovery, AirPlay, printing, or Xcode
// traffic into a generic malware signal.
const PROTECTED_SERVICE_PORTS: &[&str] = &[
    "3283", "4659", "5223", "5353", "548", "5900", "6000", "7000", "9100", "9418",
];

fn is_protected_service_port(rule: &str) -> bool {
    let mut fields = rule.split(',').map(str::trim);
    matches!(fields.next(), Some("DEST-PORT" | "SRC-PORT"))
        && fields
            .next()
            .is_some_and(|port| PROTECTED_SERVICE_PORTS.contains(&port))
}

pub fn sync_ports(ports_source: &str, firewall_modules: &[&str], execute: bool, version_str: &str) {
    println!("--- Firewall Port Sync (English Script Interface) ---");

    let source_path = Path::new(ports_source);
    if !source_path.exists() {
        println!(
            "\x1b[0;33m[WARN]\x1b[0m Port rules source not found: {}",
            ports_source
        );
        return;
    }

    let source_content = match fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(e) => {
            println!(
                "\x1b[0;31m[ERROR]\x1b[0m Failed to read ports source: {:?}",
                e
            );
            return;
        }
    };

    let mut port_rules = Vec::new();
    let mut invalid_rules = Vec::new();

    for line in source_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if PORT_PATTERN.is_match(trimmed) {
            let parts: Vec<&str> = trimmed.split(',').collect();
            if parts.len() < 3 {
                invalid_rules.push(trimmed.to_string());
                println!(
                    "\x1b[0;33m[WARN]\x1b[0m   Invalid rule (missing action): {}",
                    trimmed
                );
            } else if is_protected_service_port(trimmed) {
                println!(
                    "\x1b[0;34m[INFO]\x1b[0m   Protected service port omitted: {}",
                    trimmed
                );
            } else {
                port_rules.push(trimmed.to_string());
            }
        }
    }
    port_rules.sort();
    port_rules.dedup();

    if !invalid_rules.is_empty() {
        println!(
            "\x1b[0;33m[WARN]\x1b[0m Skipping {} invalid rules (missing action)",
            invalid_rules.len()
        );
        for rule in &invalid_rules {
            println!("\x1b[0;33m[WARN]\x1b[0m   - {}", rule);
        }
    }

    if port_rules.is_empty() {
        println!("\x1b[0;33m[WARN]\x1b[0m No port rules found in source.");
        return;
    }

    println!(
        "\x1b[0;34m[INFO]\x1b[0m Extracted {} valid rules from source.",
        port_rules.len()
    );

    let mut managed_keys = HashMap::new();
    for rule in &port_rules {
        let parts: Vec<&str> = rule.split(',').collect();
        if parts.len() >= 2 {
            let key = format!(
                "{},{}",
                parts[0].trim().to_uppercase(),
                parts[1].trim().to_uppercase()
            );
            managed_keys.insert(key, true);
        }
    }

    for module_path_str in firewall_modules {
        let module_path = Path::new(module_path_str);
        if !module_path.exists() {
            println!(
                "\x1b[0;33m[WARN]\x1b[0m Firewall module not found: {}",
                module_path_str
            );
            continue;
        }

        let module_content = match fs::read_to_string(module_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut new_module_content = Vec::new();
        let start_marker = "# --- SYNCED PORT RULES START ---";
        let end_marker = "# --- SYNCED PORT RULES END ---";
        let has_markers =
            module_content.contains(start_marker) && module_content.contains(end_marker);
        let mut skip = false;
        let mut in_rule_section = false;
        let mut inserted = false;

        for line in module_content.lines() {
            let stripped = line.trim();
            if stripped.starts_with('[') && stripped.ends_with(']') {
                in_rule_section = stripped.to_lowercase() == "[rule]";
            }

            if in_rule_section && !has_markers && !inserted {
                new_module_content.push(line.to_string());
                new_module_content.push(start_marker.to_string());
                new_module_content.push("# Automated update from ports source".to_string());
                let mut sorted_rules = port_rules.clone();
                sorted_rules.sort();
                new_module_content.extend(sorted_rules);
                new_module_content.push(end_marker.to_string());
                inserted = true;
                continue;
            }

            if line.contains(start_marker) {
                new_module_content.push(line.to_string());
                new_module_content.push("# Automated update from ports source".to_string());

                let mut sorted_rules = port_rules.clone();
                sorted_rules.sort();
                for rule in sorted_rules {
                    new_module_content.push(rule);
                }
                skip = true;
                continue;
            }

            if line.contains(end_marker) {
                skip = false;
                new_module_content.push(line.to_string());
                continue;
            }

            if !skip {
                if in_rule_section && is_protected_service_port(stripped) {
                    continue;
                }
                if in_rule_section && !stripped.is_empty() && !stripped.starts_with('#') {
                    let parts: Vec<&str> = stripped.split(',').collect();
                    if parts.len() >= 2 {
                        let key = format!(
                            "{},{}",
                            parts[0].trim().to_uppercase(),
                            parts[1].trim().to_uppercase()
                        );
                        if managed_keys.contains_key(&key) {
                            continue;
                        }
                    }
                }
                new_module_content.push(line.to_string());
            }
        }

        if !inserted && !has_markers {
            new_module_content.push(String::new());
            new_module_content.push("[Rule]".to_string());
            new_module_content.push(start_marker.to_string());
            new_module_content.push("# Automated update from ports source".to_string());
            let mut sorted_rules = port_rules.clone();
            sorted_rules.sort();
            new_module_content.extend(sorted_rules);
            new_module_content.push(end_marker.to_string());
        }

        let mut final_content = new_module_content.join("\n");
        if !final_content.ends_with('\n') {
            final_content.push('\n');
        }

        let file_name = module_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        if execute {
            let new_version = format!("#!version={}", version_str);
            final_content = VERSION_RE
                .replace_all(&final_content, new_version.as_str())
                .to_string();
            let success = crate::safe_write_file_internal(module_path, &final_content, true);
            if success {
                println!(
                    "\x1b[0;32m[✓]\x1b[0m Firewall module updated successfully: {}",
                    file_name
                );
            }
        } else {
            println!(
                "\x1b[0;34m[INFO]\x1b[0m Dry Run: Use execute=true to apply changes to {}.",
                file_name
            );
        }
    }
}

const SYSTEM_DNS_HOSTS: &[&str] = &[
    "localhost",
    "*.localhost",
    "*.local",
    "*.lan",
    "*.home.arpa",
    "*.localdomain",
    "router.asus.com",
    "routerlogin.net",
    "orbilogin.com",
    "miwifi.com",
    "www.miwifi.com",
    "tplogin.cn",
    "tplinklogin.net",
    "tplinkwifi.net",
    "melogin.cn",
    "falogin.cn",
    "tendawifi.com",
];

fn covered_by_wildcard_parent(host: &str, hosts: &BTreeSet<String>) -> bool {
    let mut candidate = host.strip_prefix("*.").unwrap_or(host);
    while let Some((_, parent)) = candidate.split_once('.') {
        if hosts.contains(&format!("*.{parent}")) {
            return true;
        }
        candidate = parent;
    }
    false
}

fn prune_redundant_system_hosts(hosts: &mut BTreeSet<String>) {
    let redundant: Vec<String> = hosts
        .iter()
        .filter(|host| covered_by_wildcard_parent(host, hosts))
        .cloned()
        .collect();
    for host in redundant {
        hosts.remove(&host);
    }
}

fn helper_sections(root: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let apple_path = root.join("rulesets/RULE-SET/Apple.list");
    let content = fs::read_to_string(&apple_path)
        .map_err(|error| format!("failed to read {}: {error}", apple_path.display()))?;
    let mut system_hosts = BTreeSet::new();
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let rule = crate::promax::rule::SurgeRule::parse(line)
            .map_err(|error| format!("invalid Apple rule {line:?}: {error}"))?;
        match rule.kind {
            crate::promax::rule::RuleKind::Domain => {
                system_hosts.insert(rule.payload);
            }
            crate::promax::rule::RuleKind::DomainSuffix => {
                system_hosts.insert(rule.payload.clone());
                system_hosts.insert(format!("*.{}", rule.payload));
            }
            crate::promax::rule::RuleKind::DomainWildcard => {
                system_hosts.insert(rule.payload);
            }
            _ => {}
        }
    }
    system_hosts.extend(SYSTEM_DNS_HOSTS.iter().map(|host| (*host).to_string()));
    prune_redundant_system_hosts(&mut system_hosts);
    system_hosts.remove("dns.alidns.com");
    let mut hosts: Vec<String> = system_hosts
        .into_iter()
        .map(|host| format!("{host} = server:system"))
        .collect();
    hosts.push("dns.alidns.com = 223.5.5.5, 223.6.6.6".to_string());
    hosts.sort();

    let exclusions = crate::promax::safety::apple_mitm_exclusions();
    Ok((hosts.into_iter().collect(), exclusions))
}

fn render_helper(
    root: &Path,
    shadowrocket: bool,
    with_firewall: bool,
    version: &str,
) -> Result<String, String> {
    let (hosts, exclusions) = helper_sections(root)?;
    let name = if with_firewall {
        "🛡️ PROMAX AdBlock Helper (DNS & Firewall)"
    } else {
        "🌟 AdBlock Helper"
    };
    let prefix = if shadowrocket { "[🚀SR] " } else { "" };
    let general = if shadowrocket {
        "fallback-dns-server = %APPEND% 223.5.5.5, 119.29.29.29, https://dns.alidns.com/dns-query"
    } else {
        "dns-server = %APPEND% 223.5.5.5, 119.29.29.29\nencrypted-dns-server = %APPEND% https://dns.alidns.com/dns-query"
    };
    let mut output = vec![
        format!("#!name={name}"),
        format!(
            "#!desc={prefix}轻量 DNS 补充 + Apple/局域网系统解析、DoH 引导与关键服务 MITM 排除。模块不注入分流策略；Apple 分流由外部规则集和用户策略组决定。{}",
            if with_firewall {
                "附带高级端口过滤；开发、矿池、IRC 等场景请按需启用。"
            } else {
                ""
            }
        ),
        "#!author=ScriptHub".to_string(),
        "#!category=『 🔝 Head Expanse › 首端扩域 』".to_string(),
        "#!tag=AdBlock, DNS, Apple Safety".to_string(),
        format!("#!version={version}"),
        String::new(),
        "[General]".to_string(),
        general.to_string(),
    ];
    if with_firewall {
        output.extend([
            String::new(),
            "[Rule]".to_string(),
            "# --- SYNCED PORT RULES START ---".to_string(),
            "# Automated update from ports source".to_string(),
            "# --- SYNCED PORT RULES END ---".to_string(),
        ]);
    }
    output.extend([String::new(), "[Host]".to_string()]);
    output.extend(hosts);
    output.extend([
        String::new(),
        "[MITM]".to_string(),
        format!("hostname = %INSERT% {}", exclusions.join(", ")),
        String::new(),
    ]);
    Ok(output.join("\n"))
}

fn render_firewall(shadowrocket: bool, version: &str) -> String {
    let prefix = if shadowrocket { "[🚀SR] " } else { "" };
    format!(
        "#!name=🔥 Firewall Port Blocker 🛡️\n#!desc={prefix}高级可选出站端口过滤。会阻断部分开发、数据库、矿池与 IRC 服务；Apple、Bonjour、AirPlay、打印和 Xcode 常用端口永久排除。\n#!author=ScriptHub\n#!category=『 🔝 Head Expanse › 首端扩域 』\n#!tag=Security, Firewall, Advanced\n#!version={version}\n\n[Rule]\n# --- SYNCED PORT RULES START ---\n# Automated update from ports source\n# --- SYNCED PORT RULES END ---\n"
    )
}

pub fn refresh_security_modules(root: &Path, version: &str) -> Result<usize, String> {
    let modules = root.join("modules");
    let outputs = [
        (
            modules.join("surge/head_expanse/🌟 AdBlock Helper .sgmodule"),
            render_helper(root, false, false, version)?,
        ),
        (
            modules.join("shadowrocket/head_expanse/🌟 AdBlock Helper .module"),
            render_helper(root, true, false, version)?,
        ),
        (
            modules
                .join("shadowrocket/head_expanse/🛡️ PROMAX AdBlock Helper (DNS & Firewall).module"),
            render_helper(root, true, true, version)?,
        ),
        (
            modules.join("surge/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"),
            render_firewall(false, version),
        ),
        (
            modules.join("shadowrocket/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.module"),
            render_firewall(true, version),
        ),
    ];
    let mut written = 0;
    for (path, content) in outputs {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if !crate::safe_write_file_internal(&path, &content, true) {
            return Err(format!("failed to refresh {}", path.display()));
        }
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::{render_helper, sync_ports};
    use std::fs;

    #[test]
    fn sync_inserts_markers_and_never_blocks_apple_service_ports() {
        let directory =
            std::env::temp_dir().join(format!("script-hub-firewall-sync-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let source = directory.join("ports.list");
        let module = directory.join("firewall.sgmodule");
        fs::write(
            &source,
            "DEST-PORT,4444,REJECT-DROP\nDEST-PORT,4444,REJECT-DROP\nDEST-PORT,5223,REJECT-DROP\n",
        )
        .unwrap();
        fs::write(
            &module,
            "#!name=test\n#!version=old\n[Rule]\nDEST-PORT,3283,REJECT\n",
        )
        .unwrap();

        sync_ports(
            &source.to_string_lossy(),
            &[&module.to_string_lossy()],
            true,
            "2026.01.01",
        );
        let actual = fs::read_to_string(&module).unwrap();
        assert!(actual.contains("# --- SYNCED PORT RULES START ---"));
        assert!(actual.contains("DEST-PORT,4444,REJECT-DROP"));
        assert_eq!(actual.matches("DEST-PORT,4444,REJECT-DROP").count(), 1);
        assert!(!actual.contains("5223"));
        assert!(!actual.contains("3283"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn helper_keeps_dns_hosts_and_mitm_without_forcing_apple_policy() {
        let root =
            std::env::temp_dir().join(format!("script-hub-helper-render-{}", std::process::id()));
        let rulesets = root.join("rulesets/RULE-SET");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&rulesets).unwrap();
        fs::write(
            rulesets.join("Apple.list"),
            "DOMAIN,account.apple.com\nDOMAIN-SUFFIX,icloud.com\nDOMAIN,a1.media.apple.test\nDOMAIN-SUFFIX,media.apple.test\n",
        )
        .unwrap();

        let helper = render_helper(&root, false, false, "test").unwrap();
        assert!(helper.contains("[Host]"));
        assert!(helper.contains("account.apple.com = server:system"));
        assert!(helper.contains("*.icloud.com = server:system"));
        assert!(helper.contains("media.apple.test = server:system"));
        assert!(helper.contains("*.media.apple.test = server:system"));
        assert!(!helper.contains("a1.media.apple.test = server:system"));
        assert!(helper.contains("dns.alidns.com = 223.5.5.5, 223.6.6.6"));
        assert!(helper.contains("-account.apple.com"));
        assert!(!helper.contains("[Rule]"));
        assert!(!helper.contains(",DIRECT"));

        let firewall_helper = render_helper(&root, true, true, "test").unwrap();
        assert!(firewall_helper.contains("[Rule]"));
        assert!(firewall_helper.contains("SYNCED PORT RULES START"));
        assert!(!firewall_helper.contains(",DIRECT"));
        fs::remove_dir_all(root).unwrap();
    }
}
