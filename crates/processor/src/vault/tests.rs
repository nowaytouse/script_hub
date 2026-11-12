use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const PASSWORD: &str = "fixture-only strong passphrase";
const CONTENT: &str = "[General]\n# fixture-private-marker\n";

fn fixture() -> Library {
    Library {
        templates: vec![Template {
            name: "Fixture Surge".to_owned(),
            client: Client::Surge,
            content: CONTENT.to_owned(),
        }],
    }
}

fn documented_sample_password(document: &str) -> &str {
    let mut lines = document.lines();
    let mut password = None;
    while let Some(line) = lines.next() {
        if line == "```text" {
            assert!(
                password.is_none(),
                "Use one public-password text block per document"
            );
            let value = lines
                .next()
                .expect("Missing public-password text block content");
            assert!(!value.is_empty(), "The public-password text block is empty");
            assert!(
                lines.next() == Some("```"),
                "The public-password block must contain exactly one line"
            );
            password = Some(value);
        }
    }
    password.expect("Missing public-password text block")
}

#[test]
fn documented_public_password_authenticates_the_sanitized_encrypted_sample() {
    const DIRECT_PROBE: &str = "http://www.msftconnecttest.com/connecttest.txt";
    const PROXY_PROBE: &str = "https://connectivitycheck.grapheneos.network/generate_204";
    let group_probe =
        regex::Regex::new(r#"(?i)(?:^|,)[ \t]*url[ \t]*=[ \t]*"?(https?://[^ \t\r\n",]+)"#)
            .unwrap();
    let english = documented_sample_password(include_str!("../../../../docs/TEMPLATES.md"));
    let chinese = documented_sample_password(include_str!("../../../../docs/TEMPLATES.zh-CN.md"));
    assert!(english == chinese, "The public-password documents disagree");
    let password = zeroize::Zeroizing::new(english.to_owned());
    let vault =
        template::unlock_for_display(include_bytes!("../../assets/templates.vault"), &password)
            .expect("The public-password documents and encrypted sample do not authenticate");
    drop(password);
    assert_eq!(vault.library().templates.len(), 4);
    for client in [
        Client::Surge,
        Client::Shadowrocket,
        Client::FlClash,
        Client::SingBox,
    ] {
        assert_eq!(
            vault
                .library()
                .templates
                .iter()
                .filter(|entry| entry.client == client)
                .count(),
            1
        );
    }
    assert!(
        validate_library(vault.library()).is_ok(),
        "The public encrypted sample is invalid"
    );
    for template in &vault.library().templates {
        // Derived sing-box content comes from the sanitized Surge source, not
        // arbitrary JSON admitted through public provisioning.
        if matches!(template.client, Client::Surge | Client::Shadowrocket) {
            let sanitized = template::sanitize(template)
                .expect("The public encrypted sample cannot pass the sanitizer");
            assert!(
                sanitized.name == template.name
                    && sanitized.client == template.client
                    && sanitized.content == template.content,
                "The public encrypted sample still requires sanitization"
            );
        }
        let subscriptions = template::subscription_slots(template)
            .into_iter()
            .map(|slot| (slot, "https://example.invalid/subscription".to_owned()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let rendered = template::render(template, &subscriptions)
            .expect("The public encrypted sample has invalid subscription slots");
        assert!(!rendered.contains("/Microsoft_extra.list"));
        assert!(!rendered.contains("/GitHub.list"));
        assert!(!rendered.contains("/Microsoft.list"));
        if matches!(template.client, Client::Surge | Client::Shadowrocket) {
            let direct = rendered.find("Microsoft_direct.list,DIRECT");
            let proxy = rendered.find("Microsoft_proxy.list,");
            assert!(
                matches!((direct, proxy), (Some(direct), Some(proxy)) if direct < proxy)
                    && !rendered.contains("/Microsoft.list,")
                    && !rendered.contains("/GitHub.list,"),
                "The public sample must export the new Microsoft routing split"
            );
        }
        if template.client == Client::FlClash {
            assert!(!subscriptions.is_empty());
            assert!(template::is_flclash_config(&rendered));
        }
        if matches!(template.client, Client::Surge | Client::Shadowrocket) {
            let mut section = "";
            let mut settings = std::collections::BTreeMap::new();
            let mut group_count = 0;
            for line in rendered.lines().map(str::trim) {
                if let Some(header) = line
                    .strip_prefix('[')
                    .and_then(|value| value.strip_suffix(']'))
                {
                    section = header;
                } else if section.eq_ignore_ascii_case("General")
                    && let Some((key, value)) = line.split_once('=')
                {
                    settings.insert(key.trim(), value.trim().trim_matches('"'));
                } else if section.eq_ignore_ascii_case("Proxy Group")
                    && !line.starts_with(['#', ';', '/'])
                {
                    for capture in group_probe.captures_iter(line) {
                        assert!(
                            &capture[1] == PROXY_PROBE,
                            "A group uses an unexpected probe"
                        );
                        group_count += 1;
                    }
                }
            }
            for (key, expected) in [
                ("internet-test-url", DIRECT_PROBE),
                ("proxy-test-url", PROXY_PROBE),
            ] {
                assert!(
                    settings.get(key) == Some(&expected),
                    "Unexpected connectivity probe: {key}"
                );
            }
            if template.client == Client::Shadowrocket {
                assert_eq!(group_count, 3);
                for (key, value) in [
                    ("ipv6", "true"),
                    ("dns-direct-system", "false"),
                    ("dns-direct-fallback-proxy", "false"),
                    (
                        "direct-dns-server",
                        "https://dns.alidns.com/dns-query, https://doh.pub/dns-query",
                    ),
                ] {
                    assert!(
                        settings.get(key) == Some(&value),
                        "Unexpected Shadowrocket setting: {key}"
                    );
                }
            }
        }
        if template.client == Client::SingBox {
            let config: serde_json::Value = serde_json::from_str(&rendered).unwrap();
            assert!(config["outbounds"].is_array());
            assert!(
                config["route"]["rule_set"]
                    .as_array()
                    .is_some_and(|rules| !rules.is_empty())
            );
            assert!(subscriptions.is_empty());
        }
    }
}

#[test]
fn opening_a_user_library_does_not_rewrite_legacy_rules() {
    let mut library = fixture();
    let template = &mut library.templates[0];
    let base = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/RULE-SET/";
    template.content = format!(
        "[Rule]\nRULE-SET,{base}Microsoft.list,PROXY\nRULE-SET,{base}GitHub.list,PROXY\nFINAL,DIRECT"
    );
    let vault = create(library, PASSWORD).unwrap();
    let encrypted = vault.seal().unwrap();
    let reopened = template::unlock_for_display(&encrypted, PASSWORD).unwrap();
    assert!(
        reopened.library().templates[0].content == vault.library().templates[0].content,
        "Opening a user-owned vault must preserve its routing"
    );
    assert!(
        template::render(&reopened.library().templates[0], &Default::default()).is_err(),
        "A retired repository rule-set URL must not be exported"
    );
}

#[test]
fn created_library_is_hidden_until_correct_password_unlocks_it() {
    let vault = create(fixture(), PASSWORD).unwrap();
    let ciphertext = vault.seal().unwrap();
    for marker in [
        b"fixture-private-marker".as_slice(),
        b"Fixture Surge".as_slice(),
    ] {
        assert!(!ciphertext.windows(marker.len()).any(|part| part == marker));
    }
    let reopened = unlock(&ciphertext, PASSWORD).unwrap();
    assert_eq!(reopened.library().templates[0].content, CONTENT);
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "script-hub-vault-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn private_file_round_trip_preserves_bytes_and_refuses_unapproved_overwrite() {
    let directory = TestDirectory::new();
    let path = directory.0.join("library.vault");
    let original = b"\0\xffnot-text\r\nexact bytes\0";
    write_private(&path, original, false).unwrap();
    assert_eq!(read_bounded(&path).unwrap(), original);
    assert_eq!(
        write_private(&path, b"replacement", false),
        Err(VaultError::AlreadyExists)
    );
    assert_eq!(read_bounded(&path).unwrap(), original);
    write_private(&path, b"replacement", true).unwrap();
    assert_eq!(read_bounded(&path).unwrap(), b"replacement");
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn json_escaping_cannot_bypass_the_plaintext_limit_at_creation() {
    let library = Library {
        templates: (0..2)
            .map(|index| Template {
                name: format!("Fixture {index}"),
                client: Client::Other,
                content: "\u{1}".repeat(MAX_TEMPLATE_BYTES),
            })
            .collect(),
    };
    assert!(matches!(
        create(library, PASSWORD),
        Err(VaultError::TooLarge)
    ));
}

#[test]
fn incorrect_password_and_authenticated_data_mutations_never_unlock() {
    let ciphertext = create(fixture(), PASSWORD).unwrap().seal().unwrap();
    assert!(matches!(
        unlock(&ciphertext, "different fixture password"),
        Err(VaultError::Authentication)
    ));
    for index in [9, 25, ciphertext.len() - 1] {
        let mut damaged = ciphertext.clone();
        damaged[index] ^= 1;
        assert!(matches!(
            unlock(&damaged, PASSWORD),
            Err(VaultError::Authentication)
        ));
    }
}

#[test]
fn every_save_has_a_fresh_nonce_and_every_creation_has_a_fresh_salt() {
    let vault = create(fixture(), PASSWORD).unwrap();
    let first = vault.seal().unwrap();
    let second = vault.seal().unwrap();
    assert_ne!(first, second);
    assert_eq!(
        unlock(&second, PASSWORD).unwrap().library().templates[0].content,
        CONTENT
    );
    let independent = create(fixture(), PASSWORD).unwrap().seal().unwrap();
    assert_ne!(&first[9..25], &independent[9..25]);
}

#[test]
fn truncation_and_unknown_versions_are_rejected_without_returning_partial_plaintext() {
    let ciphertext = create(fixture(), PASSWORD).unwrap().seal().unwrap();
    for length in [0, 8, 49, 64, ciphertext.len() - 1] {
        assert!(unlock(&ciphertext[..length], PASSWORD).is_err());
    }
    let mut unsupported = ciphertext;
    unsupported[8] = 2;
    assert!(matches!(
        unlock(&unsupported, PASSWORD),
        Err(VaultError::Format)
    ));
}

#[test]
fn oversized_untrusted_ciphertext_is_rejected_before_attempting_a_password() {
    let bytes = vec![0u8; MAX_PLAINTEXT_BYTES + 1024];
    assert!(matches!(unlock(&bytes, ""), Err(VaultError::TooLarge)));
}

#[test]
fn password_boundaries_do_not_trim_the_supplied_password() {
    assert!(matches!(
        create(fixture(), "short"),
        Err(VaultError::Password)
    ));
    assert!(matches!(
        create(fixture(), &"x".repeat(MAX_PASSWORD_BYTES + 1)),
        Err(VaultError::Password)
    ));
    let padded = "  fixture password with spaces  ";
    let ciphertext = create(fixture(), padded).unwrap().seal().unwrap();
    assert!(matches!(
        unlock(&ciphertext, padded.trim()),
        Err(VaultError::Authentication)
    ));
    assert_eq!(
        unlock(&ciphertext, padded).unwrap().library().templates[0].content,
        CONTENT
    );
}

#[test]
fn library_limits_apply_before_password_derivation_and_after_mutation() {
    let too_many = Library {
        templates: (0..=MAX_TEMPLATES)
            .map(|index| Template {
                name: format!("Fixture {index}"),
                client: Client::Other,
                content: CONTENT.to_owned(),
            })
            .collect(),
    };
    assert!(matches!(
        create(too_many, PASSWORD),
        Err(VaultError::TooLarge)
    ));
    let mut vault = create(fixture(), PASSWORD).unwrap();
    vault.library_mut().templates[0].content = "x".repeat(MAX_TEMPLATE_BYTES + 1);
    assert!(matches!(vault.seal(), Err(VaultError::TooLarge)));
}

#[test]
fn reads_refuse_oversized_files_and_missing_directories_are_not_created() {
    let directory = TestDirectory::new();
    let oversized = directory.0.join("oversized");
    fs::File::create(&oversized)
        .unwrap()
        .set_len((MAX_PLAINTEXT_BYTES + 1024) as u64)
        .unwrap();
    assert_eq!(read_bounded(&oversized), Err(VaultError::TooLarge));
    assert_eq!(
        write_private(&directory.0.join("missing/file"), b"test", false),
        Err(VaultError::Io)
    );
    assert!(!directory.0.join("missing").exists());
    assert_eq!(read_bounded(&directory.0), Err(VaultError::Io));
}

#[cfg(unix)]
#[test]
fn private_outputs_are_owner_only_including_after_atomic_replacement() {
    use std::os::unix::fs::PermissionsExt;
    let directory = TestDirectory::new();
    let path = directory.0.join("private.conf");
    write_private(&path, b"first", false).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    write_private(&path, b"second", true).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(windows)]
#[test]
fn windows_private_outputs_do_not_inherit_a_read_shared_parent_acl() {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    fn powershell(stage: &str, path: &std::path::Path, script: &str) {
        // Keep this short fixture compatible with Windows PowerShell's path provider.
        let temporary = std::env::temp_dir();
        let canonical_temporary = temporary.canonicalize().unwrap();
        let shell_path = temporary.join(path.strip_prefix(canonical_temporary).unwrap());
        assert_eq!(shell_path.canonicalize().unwrap().as_path(), path);
        let executable = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let output = Command::new(executable)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .env("SCRIPT_HUB_ACL_TEST_PATH", shell_path)
            .stdin(Stdio::null())
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        // ACL diagnostics can contain local names and paths; keep them out of logs.
        assert!(
            output.status.success(),
            "Windows ACL fixture {stage} failed (exit {:?})",
            output.status.code()
        );
    }

    let directory = TestDirectory::new();
    powershell(
        "setup",
        &directory.0,
        // Framework ACL APIs do not depend on inherited PowerShell module search paths.
        r#"$ErrorActionPreference = 'Stop'
try { $acl = [System.IO.Directory]::GetAccessControl($env:SCRIPT_HUB_ACL_TEST_PATH) } catch { exit 71 }
$everyone = [System.Security.Principal.SecurityIdentifier]::new('S-1-1-0')
$rule = [System.Security.AccessControl.FileSystemAccessRule]::new($everyone, 'ReadAndExecute', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
$acl.AddAccessRule($rule)
try { [System.IO.Directory]::SetAccessControl($env:SCRIPT_HUB_ACL_TEST_PATH, $acl) } catch { exit 72 }"#,
    );
    let path = directory.0.join("private.conf");
    const ASSERT_PRIVATE: &str = r#"$ErrorActionPreference = 'Stop'
try { $acl = [System.IO.File]::GetAccessControl($env:SCRIPT_HUB_ACL_TEST_PATH) } catch { exit 81 }
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$rules = @($acl.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier]))
if (!$acl.AreAccessRulesProtected) { exit 82 }
if ($rules.Count -ne 1) { exit 83 }
if ($rules[0].IdentityReference.Value -ne $sid) { exit 84 }
if ($rules[0].AccessControlType -ne 'Allow') { exit 85 }
if ($rules[0].IsInherited) { exit 86 }
if (($rules[0].FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::FullControl) -ne [System.Security.AccessControl.FileSystemRights]::FullControl) { exit 87 }"#;
    write_private(&path, b"fixture first private payload", false).unwrap();
    assert_eq!(
        read_bounded(&path).unwrap(),
        b"fixture first private payload"
    );
    powershell("created", &path, ASSERT_PRIVATE);
    // Replacing the already-private target must not inherit the shared parent ACL.
    write_private(&path, b"fixture replacement payload", true).unwrap();
    assert_eq!(read_bounded(&path).unwrap(), b"fixture replacement payload");
    powershell("replaced", &path, ASSERT_PRIVATE);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn symlink_leaves_and_ancestors_cannot_redirect_reads_or_writes() {
    use std::os::unix::fs::symlink;
    let directory = TestDirectory::new();
    let real = directory.0.join("real");
    fs::create_dir(&real).unwrap();
    let original = real.join("original");
    fs::write(&original, b"keep fixture unchanged").unwrap();
    let leaf = directory.0.join("leaf");
    symlink(&original, &leaf).unwrap();
    assert_eq!(read_bounded(&leaf), Err(VaultError::Io));
    assert_eq!(
        write_private(&leaf, b"replacement", true),
        Err(VaultError::Io)
    );
    let alias = directory.0.join("alias");
    symlink(&real, &alias).unwrap();
    assert_eq!(read_bounded(&alias.join("original")), Err(VaultError::Io));
    assert_eq!(
        write_private(&alias.join("new"), b"replacement", false),
        Err(VaultError::Io)
    );
    assert_eq!(fs::read(&original).unwrap(), b"keep fixture unchanged");
    assert!(!real.join("new").exists());
}
