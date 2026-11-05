use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

static PORT_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IN-PORT|DEST-PORT|SRC-PORT)(?:,|$)").unwrap());

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
            if trimmed.split(',').nth(2).is_none() {
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

    let mut managed_keys = HashSet::new();
    for rule in &port_rules {
        let mut fields = rule.split(',');
        if let (Some(rule_type), Some(port)) = (fields.next(), fields.next()) {
            let key = format!(
                "{},{}",
                rule_type.trim().to_uppercase(),
                port.trim().to_uppercase()
            );
            managed_keys.insert(key);
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
                in_rule_section = stripped.eq_ignore_ascii_case("[rule]");
            }

            if in_rule_section && !has_markers && !inserted {
                new_module_content.push(line.to_string());
                new_module_content.push(start_marker.to_string());
                new_module_content.push("# Automated update from ports source".to_string());
                new_module_content.extend(port_rules.iter().cloned());
                new_module_content.push(end_marker.to_string());
                inserted = true;
                continue;
            }

            if line.contains(start_marker) {
                new_module_content.push(line.to_string());
                new_module_content.push("# Automated update from ports source".to_string());

                new_module_content.extend(port_rules.iter().cloned());
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
                    let mut fields = stripped.split(',');
                    if let (Some(rule_type), Some(port)) = (fields.next(), fields.next()) {
                        let key = format!(
                            "{},{}",
                            rule_type.trim().to_uppercase(),
                            port.trim().to_uppercase()
                        );
                        if managed_keys.contains(&key) {
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
            new_module_content.extend(port_rules.iter().cloned());
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

// Only local namespaces and exact vendor-documented LAN management names.
// Never derive system-DNS exceptions from public service routing lists.
const LOCAL_SYSTEM_DNS_HOSTS: &[&str] = &[
    "localhost",
    "*.localhost",
    "local",
    "*.local",
    "*.lan",
    "home.arpa",
    "*.home.arpa",
    "*.localdomain",
    "router.asus.com",
    "www.asusrouter.com",
    "routerlogin.com",
    "routerlogin.net",
    "www.routerlogin.com",
    "www.routerlogin.net",
    "orbilogin.com",
    "orbilogin.net",
    "aplogin.com",
    "aplogin.net",
    "miwifi.com",
    "www.miwifi.com",
    "tplogin.cn",
    "tplinklogin.net",
    "tplinkwifi.net",
    "tplinkap.net",
    "tplinkdeco.net",
    "tplinkmifi.net",
    "tplinkmodem.net",
    "tplinkplc.net",
    "tplinkrepeater.net",
    "fritz.box",
    "melogin.cn",
    "falogin.cn",
    "tendawifi.com",
];

const HELPER_DOH_HOST: &str = "dns.alidns.com";
// Bootstrap addresses only, not additional DNS servers. https://alidns.com/
const HELPER_DOH_BOOTSTRAP: &str = "223.5.5.5, 223.6.6.6, 2400:3200::1, 2400:3200:baba::1";
const ENCRYPTED_DNS_MITM_EXCLUSIONS: &[&str] = &[
    HELPER_DOH_HOST,
    "cloudflare-dns.com",
    "dns.google",
    "dns.quad9.net",
];

fn helper_sections() -> (Vec<String>, Vec<String>) {
    let mut hosts: Vec<String> = LOCAL_SYSTEM_DNS_HOSTS
        .iter()
        .map(|host| format!("{host} = server:system"))
        .collect();
    hosts.push(format!("{HELPER_DOH_HOST} = {HELPER_DOH_BOOTSTRAP}"));
    hosts.sort_unstable();

    let mut exclusions = crate::promax::safety::apple_mitm_exclusions();
    exclusions.extend(
        ENCRYPTED_DNS_MITM_EXCLUSIONS
            .iter()
            .map(|host| format!("-{host}")),
    );
    exclusions.sort_unstable();
    exclusions.dedup();
    (hosts, exclusions)
}

fn render_helper(shadowrocket: bool, with_firewall: bool, version: &str) -> String {
    let (hosts, exclusions) = helper_sections();
    let name = if with_firewall {
        "🛡️ PROMAX AdBlock Helper (DNS & Firewall)"
    } else {
        "🌟 AdBlock Helper"
    };
    let prefix = if shadowrocket { "[🚀SR] " } else { "" };
    let dns_key = if shadowrocket {
        "fallback-dns-server"
    } else {
        "encrypted-dns-server"
    };
    let mut output = vec![
        format!("#!name={name}"),
        format!(
            "#!desc={prefix}追加 AliDNS DoH 与双栈引导；系统解析仅用于局域网域名和设备入口。保留 Apple 关键服务与加密 DNS 的 MITM 排除，不改分流策略。{}",
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
        format!("{dns_key} = %APPEND% https://{HELPER_DOH_HOST}/dns-query"),
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
    output.join("\n")
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
            render_helper(false, false, version),
        ),
        (
            modules.join("shadowrocket/head_expanse/🌟 AdBlock Helper .module"),
            render_helper(true, false, version),
        ),
        (
            modules
                .join("shadowrocket/head_expanse/🛡️ PROMAX AdBlock Helper (DNS & Firewall).module"),
            render_helper(true, true, version),
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
    use super::{HELPER_DOH_HOST, render_helper, sync_ports};
    use std::collections::HashSet;
    use std::fs;
    use std::net::IpAddr;

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
            "DEST-PORT,4444,REJECT-DROP\nDEST-PORT,3333,REJECT-DROP\nDEST-PORT,4444,REJECT-DROP\nDEST-PORT,5223,REJECT-DROP\nDEST-PORTABLE,7777,REJECT-DROP\n",
        )
        .unwrap();
        fs::write(
            &module,
            "#!name=test\n#!version=old\n[rUlE]\nDEST-PORT,3283,REJECT\n",
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
        assert!(actual.find("DEST-PORT,3333").unwrap() < actual.find("DEST-PORT,4444").unwrap());
        assert!(!actual.contains("5223"));
        assert!(!actual.contains("3283"));
        assert!(!actual.contains("DEST-PORTABLE"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn helper_only_adds_encrypted_dns_and_local_host_overrides() {
        for (shadowrocket, firewall, dns_key) in [
            (false, false, "encrypted-dns-server"),
            (true, false, "fallback-dns-server"),
            (true, true, "fallback-dns-server"),
        ] {
            let module = render_helper(shadowrocket, firewall, "test");
            let (_, body) = module.split_once("[General]\n").unwrap();
            assert_eq!(
                body.split_once("\n\n").unwrap().0,
                format!("{dns_key} = %APPEND% https://{HELPER_DOH_HOST}/dns-query")
            );
            let (_, host_section) = body.split_once("[Host]\n").unwrap();
            let (hosts, mitm) = host_section.split_once("\n\n[MITM]\n").unwrap();
            let mut names = HashSet::new();
            for (host, mapping) in hosts.lines().map(|line| line.split_once(" = ").unwrap()) {
                assert!(names.insert(host), "duplicate Host mapping: {host}");
                if host == HELPER_DOH_HOST {
                    let addresses: Vec<IpAddr> = mapping
                        .split(',')
                        .map(|ip| ip.trim().parse().unwrap())
                        .collect();
                    assert_eq!(addresses.iter().filter(|ip| ip.is_ipv4()).count(), 2);
                    assert_eq!(addresses.iter().filter(|ip| ip.is_ipv6()).count(), 2);
                } else {
                    assert_eq!(mapping, "server:system");
                    assert!(
                        !host.contains(['*', '?'])
                            || matches!(
                                host,
                                "*.localhost"
                                    | "*.local"
                                    | "*.lan"
                                    | "*.home.arpa"
                                    | "*.localdomain"
                            )
                    );
                }
            }
            for required in [
                HELPER_DOH_HOST,
                "localhost",
                "home.arpa",
                "*.home.arpa",
                "www.asusrouter.com",
                "www.routerlogin.com",
                "tplinkrepeater.net",
                "fritz.box",
            ] {
                assert!(names.contains(required), "missing Host mapping: {required}");
            }
            for public_host in [
                "account.apple.com",
                "*.apple.com",
                "*.icloud.com",
                "a1.mzstatic.com",
                "*.akamaized.net",
                "cloudflare-dns.com",
                "*.tplinkwifi.net",
            ] {
                assert!(!names.contains(public_host));
            }
            let excluded: HashSet<_> = mitm
                .trim_end()
                .strip_prefix("hostname = %INSERT% ")
                .unwrap()
                .split(", ")
                .collect();
            assert!(excluded.contains("-account.apple.com"));
            assert!(excluded.contains("-dns.alidns.com"));
            assert!(excluded.contains("-cloudflare-dns.com"));
            assert_eq!(module.contains("[Rule]"), firewall);
            assert_eq!(module.contains("SYNCED PORT RULES START"), firewall);
            assert!(!module.contains(",DIRECT"));
        }
    }
}
