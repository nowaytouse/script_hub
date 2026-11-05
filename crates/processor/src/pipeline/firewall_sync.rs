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
    "lan",
    "home.arpa",
    "*.home.arpa",
    "localdomain",
    "*.localdomain",
    "internal",
    "*.internal",
    "*.127.in-addr.arpa",
    "*.d.f.ip6.arpa",
    "*.8.e.f.ip6.arpa",
    "*.9.e.f.ip6.arpa",
    "*.a.e.f.ip6.arpa",
    "*.b.e.f.ip6.arpa",
    "router.asus.com",
    "router.synology.com",
    "www.asusrouter.com",
    "expertwifi.net",
    "my.keenetic.net",
    "routerlogin.com",
    "routerlogin.net",
    "www.routerlogin.com",
    "www.routerlogin.net",
    "orbilogin.com",
    "orbilogin.net",
    "www.orbilogin.com",
    "www.orbilogin.net",
    "aplogin.com",
    "aplogin.net",
    "miwifi.com",
    "www.miwifi.com",
    "tplogin.cn",
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

const ALIDNS_DOH_HOST: &str = "dns.alidns.com";
// Bootstrap addresses only, not additional DNS servers. https://alidns.com/
const ALIDNS_DOH_BOOTSTRAP: &str = "223.5.5.5, 223.6.6.6, 2400:3200::1, 2400:3200:baba::1";
const ENCRYPTED_DNS_MITM_EXCLUSIONS: &[&str] = &[
    ALIDNS_DOH_HOST,
    "cloudflare-dns.com",
    "dns.google",
    "dns.quad9.net",
    // Old Helper profiles may still select these endpoints; exempt them without adding resolvers.
    "adblock.dns.mullvad.net",
    "dns.digitale-gesellschaft.ch",
    "doh.applied-privacy.net",
    "doh.ffmuc.net",
    "doh.libredns.gr",
    "freedns.controld.com",
];

const HELPER_OUTPUTS: &[(&str, &str, bool, bool)] = &[
    (
        "surge/head_expanse",
        "🌟 AdBlock Helper .sgmodule",
        false,
        false,
    ),
    (
        "shadowrocket/head_expanse",
        "🌟 AdBlock Helper .module",
        true,
        false,
    ),
    (
        "shadowrocket/head_expanse",
        "🛡️ PROMAX AdBlock Helper (DNS & Firewall).module",
        true,
        true,
    ),
];

pub(super) const ALIDNS_SURGE_FILE: &str = "🔐 AliDNS DoH (Opt-In).sgmodule";
const ALIDNS_OUTPUTS: &[(&str, &str, bool)] = &[
    ("surge/head_expanse", ALIDNS_SURGE_FILE, false),
    (
        "shadowrocket/head_expanse",
        "🔐 AliDNS DoH (Opt-In).module",
        true,
    ),
];

fn helper_sections(shadowrocket: bool, upstream_hosts: &[String]) -> (Vec<String>, Vec<String>) {
    let mut hosts: Vec<String> = LOCAL_SYSTEM_DNS_HOSTS
        .iter()
        .copied()
        .chain(upstream_hosts.iter().map(String::as_str))
        // Surge already uses the system resolver library for Bonjour names.
        .filter(|host| shadowrocket || !matches!(*host, "local" | "*.local"))
        .map(|host| format!("{host} = server:system"))
        .collect();
    hosts.sort_unstable();
    hosts.dedup();

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

fn render_helper(
    shadowrocket: bool,
    with_firewall: bool,
    version: &str,
    upstream_hosts: &[String],
) -> String {
    let (hosts, exclusions) = helper_sections(shadowrocket, upstream_hosts);
    let name = if with_firewall {
        "🛡️ PROMAX AdBlock Helper (DNS & Firewall)"
    } else {
        "🌟 AdBlock Helper"
    };
    let prefix = if shadowrocket { "[🚀SR] " } else { "" };
    let mut output = vec![
        format!("#!name={name}"),
        format!(
            "#!desc={prefix}保留已有 DNS 与 DoH 设置。将指定局域网名称、设备入口和私网反查交给当前系统 DNS，并排除 Apple 关键服务和加密 DNS 的 MITM。不改分流；需要 AliDNS 时另启用 AliDNS DoH 模块。{}",
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
    if !upstream_hosts.is_empty() {
        output.push(format!(
            "# Local names selected from SukkaW/Surge (AGPL-3.0): {}",
            super::helper_hosts::SOURCE,
        ));
    }
    output.extend(hosts);
    output.extend([
        String::new(),
        "[MITM]".to_string(),
        format!("hostname = %INSERT% {}", exclusions.join(", ")),
        String::new(),
    ]);
    output.join("\n")
}

fn render_alidns(shadowrocket: bool, version: &str) -> String {
    let tls = if shadowrocket {
        ""
    } else {
        "\nencrypted-dns-skip-cert-verification = false"
    };
    let (description, setting) = if shadowrocket {
        (
            "[🚀SR] 可选：将 AliDNS 加入备用 DNS，保留已有设置。需要时单独启用。",
            "fallback-dns-server = %APPEND%",
        )
    } else {
        (
            "可选：将全局加密 DNS 列表替换为 AliDNS，不与已有服务商抢答。停用模块即可恢复配置。",
            "encrypted-dns-server =",
        )
    };
    format!(
        "#!name=🔐 AliDNS DoH (Opt-In)\n#!desc={description}\n#!author=ScriptHub\n#!category=『 🔝 Head Expanse › 首端扩域 』\n#!tag=DNS, Optional\n#!version={version}\n\n[General]\n{setting} https://{ALIDNS_DOH_HOST}/dns-query{tls}\n\n[Host]\n{ALIDNS_DOH_HOST} = {ALIDNS_DOH_BOOTSTRAP}\n\n[MITM]\nhostname = %INSERT% -{ALIDNS_DOH_HOST}\n"
    )
}

fn dns_modules<'a>(
    version: &'a str,
    upstream_hosts: &'a [String],
) -> impl Iterator<Item = (&'static str, &'static str, String)> + 'a {
    HELPER_OUTPUTS
        .iter()
        .map(move |&(dir, file, shadowrocket, firewall)| {
            (
                dir,
                file,
                render_helper(shadowrocket, firewall, version, upstream_hosts),
            )
        })
        .chain(
            ALIDNS_OUTPUTS
                .iter()
                .map(move |&(dir, file, shadowrocket)| {
                    (dir, file, render_alidns(shadowrocket, version))
                }),
        )
}

fn render_firewall(shadowrocket: bool, version: &str) -> String {
    let prefix = if shadowrocket { "[🚀SR] " } else { "" };
    format!(
        "#!name=🔥 Firewall Port Blocker 🛡️\n#!desc={prefix}高级可选出站端口过滤。会阻断部分开发、数据库、矿池与 IRC 服务；Apple、Bonjour、AirPlay、打印和 Xcode 常用端口永久排除。\n#!author=ScriptHub\n#!category=『 🔝 Head Expanse › 首端扩域 』\n#!tag=Security, Firewall, Advanced\n#!version={version}\n\n[Rule]\n# --- SYNCED PORT RULES START ---\n# Automated update from ports source\n# --- SYNCED PORT RULES END ---\n"
    )
}

pub fn refresh_security_modules(root: &Path, version: &str) -> Result<usize, String> {
    let modules = root.join("modules");
    let upstream_hosts = super::helper_hosts::load(root)?;
    let helpers = dns_modules(version, &upstream_hosts)
        .map(|(dir, file, content)| (modules.join(dir).join(file), content));
    let firewalls = [
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
    for (path, content) in helpers.chain(firewalls) {
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

fn helper_dns_settings(content: &str) -> Vec<&str> {
    let mut firewall = false;
    content
        .lines()
        .map(str::trim)
        .filter(|line| {
            if line.starts_with('[') && line.ends_with(']') {
                firewall = *line == "[Rule]";
                return true;
            }
            !firewall && !line.is_empty() && !line.starts_with('#')
        })
        .collect()
}

pub(crate) fn validate_published_helpers(root: &Path) -> Result<(), String> {
    let upstream_hosts = super::helper_hosts::load(root)?;
    for (dir, file, expected) in dns_modules("", &upstream_hosts) {
        let expected = helper_dns_settings(&expected);
        let dir = root.join("modules").join(dir);
        for path in [dir.join(file), dir.join("github").join(file)] {
            if !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
                return Err(format!(
                    "missing regular DNS helper publication: {}",
                    path.display()
                ));
            }
            let current = fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            if helper_dns_settings(&current) != expected {
                return Err(format!(
                    "DNS helper settings differ from validated generation: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ALIDNS_DOH_HOST, render_alidns, render_helper, sync_ports};
    use std::collections::HashSet;
    use std::fs;
    use std::net::IpAddr;

    #[test]
    fn publication_guard_rejects_dns_drift_in_a_helper_mirror() {
        let root =
            std::env::temp_dir().join(format!("script-hub-helper-guard-{}", std::process::id()));
        fs::create_dir_all(root.join("modules/source/local")).unwrap();
        super::super::helper_hosts::publish(&root, "# Size: 1\nDOMAIN,unifi\n### EOF ###").unwrap();
        super::refresh_security_modules(&root, "test").unwrap();
        for (dir, file, _) in super::dns_modules("test", &[]) {
            let dir = root.join("modules").join(dir);
            fs::create_dir_all(dir.join("github")).unwrap();
            fs::copy(dir.join(file), dir.join("github").join(file)).unwrap();
        }
        super::validate_published_helpers(&root).unwrap();
        let mirror = root.join("modules/surge/head_expanse/github/🌟 AdBlock Helper .sgmodule");
        let drift = fs::read_to_string(&mirror)
            .unwrap()
            .replace("[Host]\n", "[Host]\n*.example.com = server:system\n");
        fs::write(mirror, drift).unwrap();
        assert!(super::validate_published_helpers(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

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
    fn helper_preserves_the_users_dns_provider() {
        for (shadowrocket, firewall) in [(false, false), (true, false), (true, true)] {
            let module = render_helper(
                shadowrocket,
                firewall,
                "test",
                &[
                    "unifi".into(),
                    "*.16.172.in-addr.arpa".into(),
                    "home.arpa".into(),
                    "*.local".into(),
                ],
            );
            assert!(!module.contains("[General]"));
            assert!(!module.contains("use-local-host-item-for-proxy"));
            assert!(!module.contains("force-syslib"));
            let (_, host_section) = module.split_once("[Host]\n").unwrap();
            let (hosts, mitm) = host_section.split_once("\n\n[MITM]\n").unwrap();
            let mut names = HashSet::new();
            for (host, mapping) in hosts
                .lines()
                .filter(|line| !line.starts_with('#'))
                .map(|line| line.split_once(" = ").unwrap())
            {
                assert!(names.insert(host), "duplicate Host mapping: {host}");
                assert_eq!(mapping, "server:system");
                assert!(
                    !host.contains(['*', '?'])
                        || host
                            .strip_prefix("*.")
                            .is_some_and(crate::pipeline::helper_hosts::local_zone)
                );
            }
            for required in [
                "localhost",
                "home.arpa",
                "*.home.arpa",
                "www.asusrouter.com",
                "www.routerlogin.com",
                "www.orbilogin.com",
                "www.orbilogin.net",
                "tplinkrepeater.net",
                "fritz.box",
                "unifi",
                "router.synology.com",
                "expertwifi.net",
                "my.keenetic.net",
                "*.internal",
                "*.16.172.in-addr.arpa",
                "*.d.f.ip6.arpa",
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
                ALIDNS_DOH_HOST,
                "*.expertwifi.net",
                "*.keenetic.net",
                "*.tplinkwifi.net",
                "tplinklogin.net",
                "*.c.f.ip6.arpa",
            ] {
                assert!(!names.contains(public_host));
            }
            assert_eq!(names.contains("*.local"), shadowrocket);
            assert_eq!(names.contains("local"), shadowrocket);
            let excluded: HashSet<_> = mitm
                .trim_end()
                .strip_prefix("hostname = %INSERT% ")
                .unwrap()
                .split(", ")
                .collect();
            assert!(excluded.contains("-account.apple.com"));
            assert!(excluded.contains("-dns.alidns.com"));
            assert!(excluded.contains("-cloudflare-dns.com"));
            for host in [
                "adblock.dns.mullvad.net",
                "dns.digitale-gesellschaft.ch",
                "doh.applied-privacy.net",
                "doh.ffmuc.net",
                "doh.libredns.gr",
                "freedns.controld.com",
            ] {
                assert!(excluded.contains(format!("-{host}").as_str()));
                assert!(!names.contains(host));
            }
            assert_eq!(module.contains("[Rule]"), firewall);
            assert_eq!(module.contains("SYNCED PORT RULES START"), firewall);
            assert!(!module.contains(",DIRECT"));
        }
    }

    #[test]
    fn optional_alidns_has_one_encrypted_endpoint_and_exact_dual_stack_bootstrap() {
        for (shadowrocket, setting) in [
            (false, "encrypted-dns-server ="),
            (true, "fallback-dns-server = %APPEND%"),
        ] {
            let module = render_alidns(shadowrocket, "test");
            let (_, body) = module.split_once("[General]\n").unwrap();
            let (general, rest) = body.split_once("\n\n[Host]\n").unwrap();
            assert_eq!(
                general,
                format!(
                    "{setting} https://{ALIDNS_DOH_HOST}/dns-query{}",
                    if shadowrocket {
                        ""
                    } else {
                        "\nencrypted-dns-skip-cert-verification = false"
                    }
                )
            );
            let (hosts, mitm) = rest.split_once("\n\n[MITM]\n").unwrap();
            let (host, mapping) = hosts.split_once(" = ").unwrap();
            assert_eq!(host, ALIDNS_DOH_HOST);
            let addresses: Vec<IpAddr> = mapping
                .split(',')
                .map(|ip| ip.trim().parse().unwrap())
                .collect();
            assert_eq!(addresses.iter().filter(|ip| ip.is_ipv4()).count(), 2);
            assert_eq!(addresses.iter().filter(|ip| ip.is_ipv6()).count(), 2);
            assert_eq!(mitm.trim_end(), format!("hostname = %INSERT% -{host}"));
            assert!(!module.contains("[Rule]"));
            assert!(!module.contains("use-local-host-item-for-proxy"));
        }
    }
}
