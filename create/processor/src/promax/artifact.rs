use super::rule::{SurgeRule, split_fields};
use super::safety::QuarantineRecord;
use super::validation::{validate_loon_plugin, validate_surge_module};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Surge,
    Loon,
    Shadowrocket,
    ExternalRuleset,
    Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedArtifact {
    pub target: PathBuf,
    pub content: String,
    pub kind: ArtifactKind,
}

const LOON_CDN_MAX_BYTES: usize = 20_000_000;

fn validate_artifact_size(
    relative: &Path,
    kind: ArtifactKind,
    byte_len: usize,
) -> Result<(), String> {
    let relative_text = relative.to_string_lossy();
    let is_cdn_loon = kind == ArtifactKind::Loon
        && relative_text.starts_with("modules/loon/")
        && !relative_text.starts_with("modules/loon/github/");
    if is_cdn_loon && byte_len > LOON_CDN_MAX_BYTES {
        return Err(format!(
            "{} is {} bytes; CDN-facing Loon artifacts must stay at or below {} bytes",
            relative.display(),
            byte_len,
            LOON_CDN_MAX_BYTES
        ));
    }
    Ok(())
}

fn semantic_changed(target: &Path, content: &str) -> bool {
    let Ok(existing) = fs::read_to_string(target) else {
        return true;
    };
    super::super::url_rewriter::semantic_content_for_path_pub(target, &existing)
        != super::super::url_rewriter::semantic_content_for_path_pub(target, content)
}

static SHADOWROCKET_SCRIPT_PARAMS: Lazy<Vec<Regex>> = Lazy::new(|| {
    [
        r#"(?i),?\s*ability=(?:"[^"]*"|[^\s,]+)"#,
        r"(?i),?\s*engine=auto\b",
        r"(?i),?\s*engine=\{\{\{engine\}\}\}",
    ]
    .into_iter()
    .map(|pattern| Regex::new(pattern).expect("Shadowrocket script parameter regex must compile"))
    .collect()
});

pub fn shadowrocket_variant(
    root: &Path,
    surge: &GeneratedArtifact,
) -> Result<GeneratedArtifact, String> {
    if surge.kind != ArtifactKind::Surge {
        return Err("Shadowrocket conversion requires a Surge artifact".to_string());
    }
    let relative = surge
        .target
        .strip_prefix(root)
        .map_err(|_| "Surge artifact target escapes repository".to_string())?;
    let relative_text = relative.to_string_lossy();
    let converted_path = relative_text
        .replace(
            "modules/surge/head_expanse",
            "modules/shadowrocket/head_expanse",
        )
        .strip_suffix(".sgmodule")
        .map(|path| format!("{path}.module"))
        .ok_or_else(|| "Surge artifact does not use the .sgmodule suffix".to_string())?;
    Ok(GeneratedArtifact {
        target: root.join(converted_path),
        content: convert_surge_to_shadowrocket(&surge.content),
        kind: ArtifactKind::Shadowrocket,
    })
}

pub fn validate_coverage_floor(root: &Path, artifacts: &[GeneratedArtifact]) -> Result<(), String> {
    let catalog_path = root.join("rulesets/AdBlock/catalog.json");
    let Ok(catalog_content) = fs::read_to_string(&catalog_path) else {
        return Ok(());
    };
    let catalog: serde_json::Value = serde_json::from_str(&catalog_content)
        .map_err(|error| format!("cannot read previous PROMAX coverage: {error}"))?;
    let previous: u64 = catalog["shards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|shard| shard["rule_count"].as_u64())
        .sum();
    if previous == 0 {
        return Ok(());
    }

    let mut current = 0_u64;
    for artifact in artifacts
        .iter()
        .filter(|artifact| artifact.kind == ArtifactKind::ExternalRuleset)
    {
        current += artifact
            .content
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with('#') && !line.starts_with(';')
            })
            .count() as u64;
    }
    let floor = previous.saturating_mul(90) / 100;
    if current < floor {
        return Err(format!(
            "PROMAX coverage regressed from {previous} to {current} rules (minimum {floor})"
        ));
    }
    Ok(())
}

pub fn publish_artifacts(
    root: &Path,
    artifacts: &[GeneratedArtifact],
    quarantine: &[QuarantineRecord],
) -> Result<(), String> {
    let staging_root = root.join(".cache/promax-staging");
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root)
            .map_err(|error| format!("cannot reset PROMAX staging directory: {error}"))?;
    }

    for artifact in artifacts {
        let relative = artifact.target.strip_prefix(root).map_err(|_| {
            format!(
                "artifact target escapes repository: {}",
                artifact.target.display()
            )
        })?;
        validate_artifact_size(relative, artifact.kind, artifact.content.len())?;
        let staged = staging_root.join(relative);
        if let Some(parent) = staged.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create staging directory: {error}"))?;
        }
        fs::write(&staged, &artifact.content)
            .map_err(|error| format!("cannot stage {}: {error}", relative.display()))?;

        let validation_errors = match artifact.kind {
            ArtifactKind::Surge | ArtifactKind::Shadowrocket => {
                validate_surge_module(&artifact.content)
            }
            ArtifactKind::Loon => validate_loon_plugin(&artifact.content),
            ArtifactKind::ExternalRuleset => {
                super::validation::validate_external_ruleset(&artifact.content)
            }
            ArtifactKind::Metadata => {
                if artifact
                    .target
                    .extension()
                    .is_some_and(|extension| extension == "json")
                    && serde_json::from_str::<serde_json::Value>(&artifact.content).is_err()
                {
                    vec![super::validation::ValidationError {
                        line: 1,
                        code: "invalid-json",
                        message: "generated metadata is not valid JSON".to_string(),
                    }]
                } else {
                    Vec::new()
                }
            }
        };
        if !validation_errors.is_empty() {
            let summary = validation_errors
                .iter()
                .take(8)
                .map(|error| format!("line {} {}: {}", error.line, error.code, error.message))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!(
                "{} failed validation: {summary}",
                relative.display()
            ));
        }
    }

    let mut sorted_quarantine = quarantine.to_vec();
    sorted_quarantine.sort_by(|left, right| {
        (&left.source, &left.candidate, &left.reason, left.risk).cmp(&(
            &right.source,
            &right.candidate,
            &right.reason,
            right.risk,
        ))
    });
    sorted_quarantine.dedup_by(|left, right| {
        left.source == right.source
            && left.candidate == right.candidate
            && left.reason == right.reason
            && left.risk == right.risk
    });
    let quarantine_content = format!(
        "{}\n",
        serde_json::to_string_pretty(&sorted_quarantine)
            .map_err(|error| format!("cannot serialize quarantine report: {error}"))?
    );
    let quarantine_relative = Path::new("rulesets/AdBlock/quarantine.json");
    let staged_quarantine = staging_root.join(quarantine_relative);
    if let Some(parent) = staged_quarantine.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create quarantine staging directory: {error}"))?;
    }
    fs::write(&staged_quarantine, &quarantine_content)
        .map_err(|error| format!("cannot stage quarantine report: {error}"))?;

    let quarantine_target = root.join(quarantine_relative);
    let mut publications: Vec<(PathBuf, PathBuf)> = artifacts
        .iter()
        .filter(|artifact| semantic_changed(&artifact.target, &artifact.content))
        .map(|artifact| {
            let relative = artifact
                .target
                .strip_prefix(root)
                .expect("validated target");
            (artifact.target.clone(), staging_root.join(relative))
        })
        .collect();
    if semantic_changed(&quarantine_target, &quarantine_content) {
        publications.push((quarantine_target, staged_quarantine));
    }
    if publications.is_empty() {
        fs::remove_dir_all(&staging_root)
            .map_err(|error| format!("cannot clean unchanged PROMAX staging: {error}"))?;
        println!("[INFO] PROMAX semantic content unchanged; publication skipped.");
        return Ok(());
    }
    promote_transaction(root, &staging_root, &publications)
}

fn promote_transaction(
    root: &Path,
    staging_root: &Path,
    publications: &[(PathBuf, PathBuf)],
) -> Result<(), String> {
    let mut targets = HashSet::new();
    for (target, _) in publications {
        if !targets.insert(target.clone()) {
            return Err(format!(
                "duplicate PROMAX publication target: {}",
                target.display()
            ));
        }
    }

    let backup_root = staging_root.join(".backup");
    let mut backups = Vec::new();
    for (target, _) in publications {
        let relative = target.strip_prefix(root).map_err(|_| {
            format!(
                "publication target escapes repository: {}",
                target.display()
            )
        })?;
        if target.exists() {
            let backup = backup_root.join(relative);
            if let Some(parent) = backup.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create PROMAX backup directory: {error}"))?;
            }
            fs::copy(target, &backup)
                .map_err(|error| format!("cannot back up {}: {error}", target.display()))?;
            backups.push((target.clone(), Some(backup)));
        } else {
            backups.push((target.clone(), None));
        }
    }

    for (target, staged) in publications {
        if let Some(parent) = target.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            rollback(&backups);
            return Err(format!("cannot create {}: {error}", parent.display()));
        }
        if let Err(error) = fs::rename(staged, target) {
            rollback(&backups);
            return Err(format!(
                "cannot atomically publish {}: {error}",
                target.display()
            ));
        }
    }
    fs::remove_dir_all(staging_root)
        .map_err(|error| format!("PROMAX published but staging cleanup failed: {error}"))?;
    Ok(())
}

fn rollback(backups: &[(PathBuf, Option<PathBuf>)]) {
    for (target, backup) in backups.iter().rev() {
        if let Some(backup) = backup {
            let _ = fs::copy(backup, target);
        } else if target.exists() {
            let _ = fs::remove_file(target);
        }
    }
}

pub fn convert_surge_to_shadowrocket(content: &str) -> String {
    let mut output = Vec::new();
    let mut section = "";

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = &trimmed[1..trimmed.len() - 1];
            output.push(raw_line.to_string());
            continue;
        }
        if let Some(description) = trimmed.strip_prefix("#!desc=") {
            let description = description.strip_prefix("[🚀SR] ").unwrap_or(description);
            output.push(format!("#!desc=[🚀SR] {description}"));
            continue;
        }
        if section == "Rule" && !trimmed.is_empty() && !trimmed.starts_with('#') {
            if trimmed.to_ascii_uppercase().starts_with("RULE-SET,") {
                output.push(convert_rule_set(trimmed));
            } else if let Ok(mut rule) = SurgeRule::parse(trimmed) {
                rule.policy = rule
                    .policy
                    .map(|policy| shadowrocket_policy(&policy).to_string());
                rule.options.clear();
                output.push(rule.render_module("REJECT"));
            } else {
                output.push(raw_line.to_string());
            }
            continue;
        }

        let mut line = raw_line.to_string();
        if section == "Script" && !trimmed.is_empty() && !trimmed.starts_with('#') {
            for pattern in SHADOWROCKET_SCRIPT_PARAMS.iter() {
                line = pattern.replace_all(&line, "").to_string();
            }
        }
        if section != "MITM" {
            line = line.replace("%APPEND% ", "").replace("%INSERT% ", "");
        }
        for modifier in [",extended-matching", ",pre-matching", ",no-resolve"] {
            line = line.replace(modifier, "");
        }
        output.push(line);
    }

    format!("{}\n", output.join("\n").trim_end())
}

fn convert_rule_set(line: &str) -> String {
    let Ok(fields) = split_fields(line) else {
        return line.to_string();
    };
    if fields.len() < 3 {
        return line.to_string();
    }
    format!("RULE-SET,{},{}", fields[1], shadowrocket_policy(&fields[2]))
}

fn shadowrocket_policy(policy: &str) -> &str {
    match policy.to_ascii_uppercase().as_str() {
        "REJECT-DROP" | "REJECT-NO-DROP" | "REJECT-TINYGIF" | "REJECT-IMG" => "REJECT",
        _ => policy,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactKind, GeneratedArtifact, convert_surge_to_shadowrocket, publish_artifacts,
        validate_artifact_size,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn shadowrocket_conversion_removes_surge_only_rule_options() {
        let source = r#"#!name=PROMAX
#!desc=fixture
[Rule]
RULE-SET,https://example.test/ad.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve
URL-REGEX,"^https://example.test/(a{1,3}|b)",REJECT,extended-matching
[Script]
fixture = type=http-response,pattern=^https://example.test/ad,script-path=https://example.test/a.js,ability="wifi",engine=auto
"#;

        let converted = convert_surge_to_shadowrocket(source);

        assert!(converted.contains("#!desc=[🚀SR] fixture"));
        assert!(converted.contains("RULE-SET,https://example.test/ad.list,REJECT"));
        assert!(converted.contains("URL-REGEX,\"^https://example.test/(a{1,3}|b)\",REJECT"));
        assert!(!converted.contains("extended-matching"));
        assert!(!converted.contains("pre-matching"));
        assert!(!converted.contains("update-interval="));
        assert!(!converted.contains("no-resolve"));
        assert!(!converted.contains("ability="));
        assert!(!converted.contains("engine=auto"));
    }

    #[test]
    fn malformed_staged_module_never_replaces_published_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-artifact-{unique}"));
        let target = root.join("modules/surge/promax.sgmodule");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "stable\n").unwrap();

        let artifacts = vec![GeneratedArtifact {
            target: target.clone(),
            content: "#!name=PROMAX\n[Rule]\nURL-REGEX,\"\"^https://bad/d\",REJECT\",REJECT\n"
                .to_string(),
            kind: ArtifactKind::Surge,
        }];

        assert!(publish_artifacts(&root, &artifacts, &[]).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "stable\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn coverage_regression_blocks_publication() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-coverage-{unique}"));
        fs::create_dir_all(root.join("rulesets/AdBlock")).unwrap();
        fs::write(
            root.join("rulesets/AdBlock/catalog.json"),
            r#"{"shards":[{"rule_count":100}]}"#,
        )
        .unwrap();
        let rules = (0..80)
            .map(|index| format!("DOMAIN,ad{index}.example"))
            .collect::<Vec<_>>()
            .join("\n");
        let artifacts = vec![GeneratedArtifact {
            target: root.join("rulesets/AdBlock/new.list"),
            content: rules,
            kind: ArtifactKind::ExternalRuleset,
        }];

        assert!(super::validate_coverage_floor(&root, &artifacts).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cdn_loon_product_has_a_hard_twenty_megabyte_budget() {
        let cdn = Path::new("modules/loon/promax.plugin");
        let github = Path::new("modules/loon/github/promax.plugin");
        assert!(validate_artifact_size(cdn, ArtifactKind::Loon, 20_000_000).is_ok());
        assert!(validate_artifact_size(cdn, ArtifactKind::Loon, 20_000_001).is_err());
        assert!(validate_artifact_size(github, ArtifactKind::Loon, 20_000_001).is_ok());
    }

    #[test]
    fn semantic_noop_does_not_touch_published_artifact() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-noop-{unique}"));
        let target = root.join("rulesets/AdBlock/rules.list");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let old = "# generated 2026-08-11\nDOMAIN-SUFFIX,alpha.example.com\nDOMAIN-SUFFIX,beta.example.com\n";
        fs::write(&target, old).unwrap();
        let artifacts = vec![GeneratedArtifact {
            target: target.clone(),
            content: "# generated 2026-08-12\n# refreshed\nDOMAIN-SUFFIX,beta.example.com\nDOMAIN-SUFFIX,alpha.example.com\n"
                .to_string(),
            kind: ArtifactKind::ExternalRuleset,
        }];

        publish_artifacts(&root, &artifacts, &[]).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), old);
        assert!(!root.join(".cache/promax-staging").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn semantic_noop_does_not_publish_module_date_only_change() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-module-noop-{unique}"));
        let target = root.join("modules/surge/promax.sgmodule");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let old = "#!name=PROMAX - [2026-08-11]\n#!version=2026.06.28\n#!date=2026-08-11 10:00:00\n[Rule]\nDOMAIN-SUFFIX,alpha.example.com,REJECT\n";
        fs::write(&target, old).unwrap();
        let artifacts = vec![GeneratedArtifact {
            target: target.clone(),
            content: "#!name=PROMAX - [2026-08-12]\n#!version=2026.08.12\n#!date=2026-08-12 10:00:00\n[Rule]\nDOMAIN-SUFFIX,alpha.example.com,REJECT\n".to_string(),
            kind: ArtifactKind::Surge,
        }];

        publish_artifacts(&root, &artifacts, &[]).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), old);
        fs::remove_dir_all(root).unwrap();
    }
}
