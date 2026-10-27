use super::rule::{SurgeRule, split_fields};
use super::safety::QuarantineRecord;
use super::validation::{
    validate_loon_plugin, validate_shadowrocket_module, validate_surge_module,
};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
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
    let surge_directory = Path::new("modules").join("surge").join("head_expanse");
    let mut module_path = relative
        .strip_prefix(&surge_directory)
        .map_err(|_| "Surge artifact is outside its expected publication directory".to_string())?
        .to_path_buf();
    if module_path
        .extension()
        .is_none_or(|extension| extension != "sgmodule")
    {
        return Err("Surge artifact does not use the .sgmodule suffix".to_string());
    }
    module_path.set_extension("module");
    Ok(GeneratedArtifact {
        target: root
            .join("modules")
            .join("shadowrocket")
            .join("head_expanse")
            .join(module_path),
        content: convert_surge_to_shadowrocket(&surge.content),
        kind: ArtifactKind::Shadowrocket,
    })
}

pub fn validate_coverage_floor(root: &Path, artifacts: &[GeneratedArtifact]) -> Result<(), String> {
    validate_coverage_floor_with_rolling(root, artifacts, &BTreeMap::new())
}

pub(crate) fn validate_coverage_floor_with_rolling(
    root: &Path,
    artifacts: &[GeneratedArtifact],
    verified_rolling_feeds: &BTreeMap<String, super::feed::VerifiedFeed>,
) -> Result<(), String> {
    let catalog_path = root.join("rulesets/AdBlock/catalog.json");
    let catalog_content = match fs::read_to_string(&catalog_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "{\"shards\":[]}".to_string(),
        Err(error) => {
            return Err(format!(
                "cannot read previous PROMAX coverage at {}: {error}",
                catalog_path.display()
            ));
        }
    };
    let catalog: serde_json::Value = serde_json::from_str(&catalog_content)
        .map_err(|error| format!("cannot read previous PROMAX coverage: {error}"))?;
    let shards = catalog["shards"]
        .as_array()
        .ok_or_else(|| "previous PROMAX coverage is missing a shards array".to_string())?;
    let mut previous_rolling = std::collections::BTreeMap::<String, u64>::new();
    let mut previous_total = 0_u64;
    let previous = shards.iter().enumerate().try_fold(
        0_u64,
        |total, (index, shard)| -> Result<u64, String> {
            let count = shard["rule_count"].as_u64().ok_or_else(|| {
                format!("previous PROMAX coverage shard {index} has an invalid rule_count")
            })?;
            previous_total = previous_total.checked_add(count).ok_or_else(|| {
                "previous PROMAX coverage rule_count total overflows u64".to_string()
            })?;
            if let Some(category) = shard["category"].as_str()
                && verified_rolling_feeds.contains_key(category)
            {
                let previous = previous_rolling.entry(category.to_string()).or_default();
                *previous = previous
                    .checked_add(count)
                    .ok_or_else(|| "previous rolling coverage count overflows u64".to_string())?;
                return Ok(total);
            }
            total.checked_add(count).ok_or_else(|| {
                "previous PROMAX coverage rule_count total overflows u64".to_string()
            })
        },
    )?;
    if previous_total == 0 && verified_rolling_feeds.is_empty() {
        return Ok(());
    }

    let mut current = 0_u64;
    let mut current_rolling = std::collections::BTreeMap::<String, u64>::new();
    let mut published_suffixes = HashSet::<&str>::new();
    let mut counted_shards = HashSet::new();
    for artifact in artifacts
        .iter()
        .filter(|artifact| artifact.kind == ArtifactKind::ExternalRuleset)
    {
        let Ok(relative) = artifact.target.strip_prefix(root) else {
            continue;
        };
        let Some(filename) = relative.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if relative.parent() != Some(Path::new("rulesets/AdBlock"))
            || !filename.starts_with("AdBlock_")
            || !filename.ends_with(".list")
            || !counted_shards.insert(relative.to_path_buf())
        {
            continue;
        }
        let count = artifact
            .content
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with('#') && !line.starts_with(';')
            })
            .count();
        let count = u64::try_from(count)
            .map_err(|_| "PROMAX coverage rule count exceeds u64".to_string())?;
        for line in artifact
            .content
            .lines()
            .filter(|_| !verified_rolling_feeds.is_empty())
        {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Ok(rule) = SurgeRule::parse(line)
                && rule.kind == super::rule::RuleKind::DomainSuffix
                && rule.policy.is_none()
                && let Some((_, payload)) = line.split_once(',')
            {
                let suffix = payload.split(',').next().unwrap_or_default().trim();
                if !suffix.is_empty() {
                    published_suffixes.insert(suffix);
                }
            }
        }
        if let Some(category) = artifact
            .content
            .lines()
            .find_map(|line| line.strip_prefix("# Category-Key: "))
            && verified_rolling_feeds.contains_key(category)
        {
            let current = current_rolling.entry(category.to_string()).or_default();
            *current = current
                .checked_add(count)
                .ok_or_else(|| "rolling coverage count overflows u64".to_string())?;
            continue;
        }
        current = current
            .checked_add(count)
            .ok_or_else(|| "PROMAX coverage rule count total overflows u64".to_string())?;
    }
    for (category, feed) in verified_rolling_feeds {
        if feed.domains.len() != feed.declared_entries {
            return Err(format!(
                "verified rolling category {category} has {} source-domain proofs for {} declared entries",
                feed.domains.len(),
                feed.declared_entries
            ));
        }
        let current = current_rolling.get(category).copied().unwrap_or_default();
        // Count effective source coverage, not ownership of globally deduplicated shards.
        let floor = (feed.declared_entries as u128 * 80).div_ceil(100);
        let covered = feed
            .domains
            .iter()
            .filter(|domain| {
                let mut candidate = domain.as_str();
                loop {
                    if published_suffixes.contains(candidate) {
                        return true;
                    }
                    let Some((_, parent)) = candidate.split_once('.') else {
                        return false;
                    };
                    candidate = parent;
                }
            })
            .count();
        if (covered as u128) < floor {
            return Err(format!(
                "verified rolling category {category} has {covered} of {} source suffix domains covered by canonical staged rules (minimum {floor}); own shard contains {current} rules",
                feed.declared_entries
            ));
        }
        println!(
            "[INFO] Verified rolling feed {category}: effective suffix coverage {covered}/{} source domains; own shard {} -> {current} rules.",
            feed.declared_entries,
            previous_rolling.get(category).copied().unwrap_or_default()
        );
    }
    let floor = (u128::from(previous) * 90 / 100) as u64;
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
    publish_artifacts_inner(root, artifacts, Some(quarantine))
}

pub(crate) fn publish_generated_artifacts(
    root: &Path,
    artifacts: &[GeneratedArtifact],
) -> Result<(), String> {
    publish_artifacts_inner(root, artifacts, None)
}

fn publish_artifacts_inner(
    root: &Path,
    artifacts: &[GeneratedArtifact],
    quarantine: Option<&[QuarantineRecord]>,
) -> Result<(), String> {
    let staging_root = root.join(".cache/promax-staging");
    if staging_root.exists() {
        if staging_root.join(".recovery-required").exists() || staging_root.join(".backup").exists()
        {
            return Err(format!(
                "PROMAX staging or recovery data must be reviewed before another publication: {}",
                staging_root.display()
            ));
        }
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
            ArtifactKind::Surge => validate_surge_module(&artifact.content),
            ArtifactKind::Shadowrocket => validate_shadowrocket_module(&artifact.content),
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
    if let Some(quarantine) = quarantine {
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
        if semantic_changed(&quarantine_target, &quarantine_content) {
            publications.push((quarantine_target, staged_quarantine));
        }
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
    fs::create_dir_all(&backup_root)
        .map_err(|error| format!("cannot create PROMAX backup directory: {error}"))?;
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
            return abort_promotion(
                staging_root,
                &backups,
                format!("cannot create {}: {error}", parent.display()),
            );
        }
        if let Err(error) = fs::rename(staged, target) {
            return abort_promotion(
                staging_root,
                &backups,
                format!("cannot atomically publish {}: {error}", target.display()),
            );
        }
    }
    if let Err(error) = fs::remove_dir_all(staging_root) {
        eprintln!("[WARN] artifacts published but staging cleanup failed: {error}");
    }
    Ok(())
}

fn abort_promotion(
    staging_root: &Path,
    backups: &[(PathBuf, Option<PathBuf>)],
    publish_error: String,
) -> Result<(), String> {
    let rollback_errors = rollback(backups);
    if !rollback_errors.is_empty() {
        let marker = staging_root.join(".recovery-required");
        let marker_error = fs::write(&marker, rollback_errors.join("\n"))
            .err()
            .map(|error| format!("; cannot write recovery marker: {error}"))
            .unwrap_or_default();
        return Err(format!(
            "{publish_error}; rollback failed: {}; recovery data preserved at {}{}",
            rollback_errors.join("; "),
            staging_root.display(),
            marker_error
        ));
    }
    if let Err(error) = fs::remove_dir_all(staging_root) {
        return Err(format!(
            "{publish_error}; rollback completed, but staging cleanup failed at {}: {error}",
            staging_root.display()
        ));
    }
    Err(publish_error)
}

fn rollback(backups: &[(PathBuf, Option<PathBuf>)]) -> Vec<String> {
    let mut errors = Vec::new();
    for (target, backup) in backups.iter().rev() {
        let result = match backup {
            Some(backup) => fs::copy(backup, target).map(|_| ()).map_err(|error| {
                format!(
                    "cannot restore {} from {}: {error}",
                    target.display(),
                    backup.display()
                )
            }),
            None => match fs::remove_file(target) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!(
                    "cannot remove partially published {}: {error}",
                    target.display()
                )),
            },
        };
        if let Err(error) = result {
            errors.push(error);
        }
    }
    errors
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
        shadowrocket_variant, validate_artifact_size, validate_coverage_floor,
    };
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
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
    fn shadowrocket_variant_maps_native_cdn_and_github_paths() {
        let root = Path::new("fixture-root");
        for mirror in [None, Some("github")] {
            let mut surge_target = root.join("modules").join("surge").join("head_expanse");
            let mut expected = root
                .join("modules")
                .join("shadowrocket")
                .join("head_expanse");
            if let Some(mirror) = mirror {
                surge_target.push(mirror);
                expected.push(mirror);
            }
            surge_target.push("PROMAX.sgmodule");
            expected.push("PROMAX.module");

            let converted = shadowrocket_variant(
                root,
                &GeneratedArtifact {
                    target: surge_target,
                    content: "#!name=PROMAX\n[Rule]\nDOMAIN,example.test,REJECT\n".to_string(),
                    kind: ArtifactKind::Surge,
                },
            )
            .unwrap();
            assert_eq!(converted.target, expected);
        }
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
    fn failed_rollback_reports_error_and_preserves_recovery_data() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-rollback-failure-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("removed-parent/artifact.list");
        let staging = root.join(".cache/promax-staging");
        let backup = staging.join(".backup/artifact.list");
        fs::create_dir_all(backup.parent().unwrap()).unwrap();
        fs::write(&backup, "last-known-good\n").unwrap();

        let error = super::abort_promotion(
            &staging,
            &[(target.clone(), Some(backup.clone()))],
            "publish failed".to_string(),
        )
        .unwrap_err();

        assert!(error.contains("publish failed"));
        assert!(error.contains(&target.display().to_string()));
        assert!(error.contains(&backup.display().to_string()));
        assert!(error.contains(&staging.display().to_string()));
        assert!(backup.is_file());
        assert!(staging.join(".recovery-required").is_file());
        assert!(super::publish_artifacts_inner(&root, &[], None).is_err());
        assert!(backup.is_file());
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
            target: root.join("rulesets/AdBlock/AdBlock_Advertising.list"),
            content: rules,
            kind: ArtifactKind::ExternalRuleset,
        }];

        assert!(super::validate_coverage_floor(&root, &artifacts).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn coverage_floor_excludes_github_mirror_rulesets() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-coverage-mirrors-{unique}"));
        fs::create_dir_all(root.join("rulesets/AdBlock")).unwrap();
        fs::write(
            root.join("rulesets/AdBlock/catalog.json"),
            r#"{"shards":[{"file":"rulesets/AdBlock/AdBlock_Advertising.list","rule_count":100}]}"#,
        )
        .unwrap();
        let rules = (0..89)
            .map(|index| format!("DOMAIN,ad{index}.example"))
            .collect::<Vec<_>>()
            .join("\n");
        let artifacts = [
            GeneratedArtifact {
                target: root.join("rulesets/AdBlock/AdBlock_Advertising.list"),
                content: rules.clone(),
                kind: ArtifactKind::ExternalRuleset,
            },
            GeneratedArtifact {
                target: root.join("rulesets/AdBlock/github/AdBlock_Advertising.list"),
                content: rules,
                kind: ArtifactKind::ExternalRuleset,
            },
        ];

        let error = validate_coverage_floor(&root, &artifacts).unwrap_err();
        assert!(error.contains("from 100 to 89 rules (minimum 90)"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_rolling_feed_can_expire_without_weakening_stable_coverage() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-rolling-coverage-{unique}"));
        let catalog = root.join("rulesets/AdBlock/catalog.json");
        fs::create_dir_all(catalog.parent().unwrap()).unwrap();
        fs::write(catalog, r#"{"shards":[{"category":"Advertising","rule_count":100},{"category":"ThreatIntel_TIF","rule_count":1000}]}"#).unwrap();
        let make_rules = |count| {
            (0..count)
                .map(|index| format!("DOMAIN,ad{index}.example\n"))
                .collect::<String>()
        };
        let mut artifacts = vec![
            GeneratedArtifact {
                target: root.join("rulesets/AdBlock/AdBlock_Advertising.list"),
                content: make_rules(100),
                kind: ArtifactKind::ExternalRuleset,
            },
            GeneratedArtifact {
                target: root.join("rulesets/AdBlock/AdBlock_ThreatIntel_TIF.list"),
                content: format!(
                    "# Category-Key: ThreatIntel_TIF\n{}",
                    (0..500)
                        .map(|index| format!("DOMAIN-SUFFIX,tif{index}.example\n"))
                        .collect::<String>()
                ),
                kind: ArtifactKind::ExternalRuleset,
            },
        ];
        let verified = std::collections::BTreeMap::from([(
            "ThreatIntel_TIF".to_string(),
            super::super::feed::VerifiedFeed {
                source: "fixture".to_string(),
                version: "fixture".to_string(),
                declared_entries: 600,
                domains: Arc::new(
                    (0..600)
                        .map(|index| format!("tif{index}.example"))
                        .collect(),
                ),
            },
        )]);
        assert!(validate_coverage_floor(&root, &artifacts).is_err());
        assert!(super::validate_coverage_floor_with_rolling(&root, &artifacts, &verified).is_ok());
        artifacts[0].content = make_rules(89);
        assert!(super::validate_coverage_floor_with_rolling(&root, &artifacts, &verified).is_err());
        artifacts[0].content = make_rules(100);
        artifacts[1].content =
            "# Category-Key: ThreatIntel_TIF\nDOMAIN-SUFFIX,tif0.example\n".to_string();
        assert!(super::validate_coverage_floor_with_rolling(&root, &artifacts, &verified).is_err());
        artifacts.pop();
        assert!(super::validate_coverage_floor_with_rolling(&root, &artifacts, &verified).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_coverage_catalog_allows_first_generation() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-coverage-first-run-{unique}"));

        assert!(validate_coverage_floor(&root, &[]).is_ok());
    }

    #[test]
    fn verified_rolling_feed_floor_applies_without_a_historical_baseline() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-rolling-first-run-{unique}"));
        let verified = std::collections::BTreeMap::from([(
            "ThreatIntel_TIF".to_string(),
            super::super::feed::VerifiedFeed {
                source: "fixture".to_string(),
                version: "fixture".to_string(),
                declared_entries: 10,
                domains: Arc::new((0..10).map(|index| format!("tif{index}.example")).collect()),
            },
        )]);
        let artifact = |count| GeneratedArtifact {
            target: root.join("rulesets/AdBlock/AdBlock_ThreatIntel_TIF.list"),
            content: format!(
                "# Category-Key: ThreatIntel_TIF\n{}",
                (0..count)
                    .map(|index| format!("DOMAIN-SUFFIX,tif{index}.example\n"))
                    .collect::<String>()
            ),
            kind: ArtifactKind::ExternalRuleset,
        };

        assert!(
            super::validate_coverage_floor_with_rolling(&root, &[artifact(1)], &verified).is_err()
        );
        assert!(
            super::validate_coverage_floor_with_rolling(&root, &[artifact(8)], &verified).is_ok()
        );
    }

    #[test]
    fn verified_tif_coverage_counts_domain_suffixes_across_canonical_categories() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-tif-cross-category-{unique}"));
        let verified = std::collections::BTreeMap::from([(
            "ThreatIntel_TIF".to_string(),
            super::super::feed::VerifiedFeed {
                source: "fixture".to_string(),
                version: "fixture".to_string(),
                declared_entries: 10,
                domains: Arc::new((0..10).map(|index| format!("tif{index}.example")).collect()),
            },
        )]);
        let artifact = |category: &str, content: String| GeneratedArtifact {
            target: root.join(format!("rulesets/AdBlock/AdBlock_{category}.list")),
            content,
            kind: ArtifactKind::ExternalRuleset,
        };
        let suffix = |indices: std::ops::Range<usize>| {
            indices
                .map(|index| format!("DOMAIN-SUFFIX,tif{index}.example\n"))
                .collect::<String>()
        };
        let covered_across_categories = [
            artifact(
                "ThreatIntel_TIF",
                format!("# Category-Key: ThreatIntel_TIF\n{}", suffix(0..6)),
            ),
            artifact("Advertising", suffix(6..8)),
        ];
        assert!(
            super::validate_coverage_floor_with_rolling(
                &root,
                &covered_across_categories,
                &verified,
            )
            .is_ok()
        );
        let mut missing_proof = verified.clone();
        missing_proof.get_mut("ThreatIntel_TIF").unwrap().domains = Arc::new(Default::default());
        assert!(
            super::validate_coverage_floor_with_rolling(
                &root,
                &covered_across_categories,
                &missing_proof
            )
            .is_err()
        );

        let unrelated = [
            artifact(
                "ThreatIntel_TIF",
                format!(
                    "# Category-Key: ThreatIntel_TIF\n{}",
                    (0..8)
                        .map(|index| format!("DOMAIN-SUFFIX,other{index}.example\n"))
                        .collect::<String>()
                ),
            ),
            artifact("Advertising", String::new()),
        ];
        assert!(super::validate_coverage_floor_with_rolling(&root, &unrelated, &verified).is_err());

        let exact_only = [artifact(
            "ThreatIntel_TIF",
            format!(
                "# Category-Key: ThreatIntel_TIF\n{}",
                (0..8)
                    .map(|index| format!("DOMAIN,tif{index}.example\n"))
                    .collect::<String>()
            ),
        )];
        assert!(
            super::validate_coverage_floor_with_rolling(&root, &exact_only, &verified).is_err()
        );

        let missing_published_suffix = [artifact(
            "ThreatIntel_TIF",
            format!("# Category-Key: ThreatIntel_TIF\n{}", suffix(0..7)),
        )];
        assert!(
            super::validate_coverage_floor_with_rolling(
                &root,
                &missing_published_suffix,
                &verified,
            )
            .is_err()
        );
    }

    #[test]
    fn verified_rolling_feed_floor_applies_to_zero_rule_baseline() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-rolling-zero-baseline-{unique}"));
        let catalog = root.join("rulesets/AdBlock/catalog.json");
        fs::create_dir_all(catalog.parent().unwrap()).unwrap();
        fs::write(&catalog, r#"{"shards":[]}"#).unwrap();
        let verified = std::collections::BTreeMap::from([(
            "ThreatIntel_TIF".to_string(),
            super::super::feed::VerifiedFeed {
                source: "fixture".to_string(),
                version: "fixture".to_string(),
                declared_entries: 10,
                domains: Arc::new((0..10).map(|index| format!("tif{index}.example")).collect()),
            },
        )]);
        let artifact = |count| GeneratedArtifact {
            target: root.join("rulesets/AdBlock/AdBlock_ThreatIntel_TIF.list"),
            content: format!(
                "# Category-Key: ThreatIntel_TIF\n{}",
                (0..count)
                    .map(|index| format!("DOMAIN-SUFFIX,tif{index}.example\n"))
                    .collect::<String>()
            ),
            kind: ArtifactKind::ExternalRuleset,
        };

        assert!(
            super::validate_coverage_floor_with_rolling(&root, &[artifact(1)], &verified).is_err()
        );
        assert!(
            super::validate_coverage_floor_with_rolling(&root, &[artifact(8)], &verified).is_ok()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unreadable_coverage_catalog_does_not_disable_the_floor() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-coverage-read-error-{unique}"));
        fs::create_dir_all(root.join("rulesets/AdBlock/catalog.json")).unwrap();

        assert!(validate_coverage_floor(&root, &[]).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_coverage_shards_or_counts_are_rejected() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-coverage-schema-{unique}"));
        let catalog = root.join("rulesets/AdBlock/catalog.json");
        fs::create_dir_all(catalog.parent().unwrap()).unwrap();

        for contents in [r#"{}"#, r#"{"shards":[{"rule_count":"100"}]}"#] {
            fs::write(&catalog, contents).unwrap();
            assert!(validate_coverage_floor(&root, &[]).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn coverage_count_overflow_is_rejected() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("promax-coverage-overflow-{unique}"));
        let catalog = root.join("rulesets/AdBlock/catalog.json");
        fs::create_dir_all(catalog.parent().unwrap()).unwrap();
        fs::write(
            &catalog,
            r#"{"shards":[{"rule_count":18446744073709551615},{"rule_count":1}]}"#,
        )
        .unwrap();

        assert!(validate_coverage_floor(&root, &[]).is_err());
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
