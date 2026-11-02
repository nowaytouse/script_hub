//! Portable plaintext node backup. Excludes GitHub tokens and security opt-ins.

use super::{
    RelayConfig,
    gist::{GistFile, validate_filename, validate_name},
    model::Subscription,
};
use serde::{Deserialize, Deserializer, Serialize};
use zeroize::{Zeroize, Zeroizing};

const MAX_CONTENT: usize = 4 * 1024 * 1024;
const MAX_FILE: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum BackupError {
    Invalid,
    TooLarge,
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "Invalid relay backup",
            Self::TooLarge => "Relay backup exceeds size limit",
        })
    }
}
impl std::error::Error for BackupError {}

pub struct RelayBackup {
    pub config: RelayConfig,
    pub content: Zeroizing<String>,
}

impl Drop for RelayBackup {
    fn drop(&mut self) {
        self.config.source.zeroize();
        self.config.source_file.zeroize();
        self.config.output.zeroize();
        self.config.output_file.zeroize();
        self.config.title.zeroize();
    }
}

#[derive(Serialize)]
struct Export<'a> {
    format: &'static str,
    version: u8,
    source: &'a str,
    source_file: &'a str,
    output: &'a str,
    output_file: &'a str,
    title: &'a str,
    rename: bool,
    sort: bool,
    concurrency: usize,
    timeout_seconds: u64,
    processing: &'a super::model::editor::Options,
    content: &'a str,
}

fn secret<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(deserializer).map(Zeroizing::new)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Import {
    #[serde(deserialize_with = "secret")]
    format: Zeroizing<String>,
    version: u8,
    #[serde(deserialize_with = "secret")]
    source: Zeroizing<String>,
    #[serde(deserialize_with = "secret")]
    source_file: Zeroizing<String>,
    #[serde(deserialize_with = "secret")]
    output: Zeroizing<String>,
    #[serde(deserialize_with = "secret")]
    output_file: Zeroizing<String>,
    #[serde(deserialize_with = "secret")]
    title: Zeroizing<String>,
    rename: bool,
    sort: bool,
    concurrency: usize,
    timeout_seconds: u64,
    #[serde(default)]
    processing: Option<super::model::editor::Options>,
    #[serde(deserialize_with = "secret")]
    content: Zeroizing<String>,
}

fn validate(config: &RelayConfig, content: &str) -> Result<(), BackupError> {
    if content.len() > MAX_CONTENT {
        return Err(BackupError::TooLarge);
    }
    let subscription = Subscription::parse(content).map_err(|_| BackupError::Invalid)?;
    subscription
        .render_processed_with_resolver(
            &vec![super::model::Health::Unknown; subscription.nodes().len()],
            &super::model::EnhanceOptions {
                rename: config.rename,
                sort: config.sort,
            },
            &config.processing,
            // Validate the plan without contacting a resolver or restoring network consent.
            &|hosts, _, _| Ok(super::model::dns::Batch::addresses(vec![None; hosts.len()])),
        )
        .map_err(|_| BackupError::Invalid)?;
    validate_name(&config.title).map_err(|_| BackupError::Invalid)?;
    if !config.source_file.is_empty() {
        validate_filename(&config.source_file).map_err(|_| BackupError::Invalid)?;
    }
    if !config.output_file.is_empty() {
        validate_filename(&config.output_file).map_err(|_| BackupError::Invalid)?;
    }
    if !(1..=8).contains(&config.concurrency) || !(1..=20).contains(&config.timeout_seconds) {
        return Err(BackupError::Invalid);
    }
    let source = if config.source.trim().is_empty() {
        None
    } else {
        Some(
            GistFile::parse_source(config.source.trim(), &config.source_file)
                .map_err(|_| BackupError::Invalid)?,
        )
    };
    if !config.output.trim().is_empty() {
        let output = GistFile::parse(config.output.trim(), &config.output_file)
            .map_err(|_| BackupError::Invalid)?;
        if source.as_ref().is_some_and(|source| source.id == output.id) {
            return Err(BackupError::Invalid);
        }
    }
    Ok(())
}

pub fn encode(config: &RelayConfig, content: &str) -> Result<Zeroizing<Vec<u8>>, BackupError> {
    let content = config
        .expected_source
        .as_ref()
        .map_or(content, |source| source.as_str());
    validate(config, content)?;
    let document = Export {
        format: "script-hub-relay",
        version: 2,
        source: &config.source,
        source_file: &config.source_file,
        output: &config.output,
        output_file: &config.output_file,
        title: &config.title,
        rename: config.rename,
        sort: config.sort,
        concurrency: config.concurrency,
        timeout_seconds: config.timeout_seconds,
        processing: &config.processing,
        content,
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&document).map_err(|_| BackupError::Invalid)?);
    if bytes.len() > MAX_FILE {
        return Err(BackupError::TooLarge);
    }
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<RelayBackup, BackupError> {
    if bytes.len() > MAX_FILE {
        return Err(BackupError::TooLarge);
    }
    let mut doc: Import = serde_json::from_slice(bytes).map_err(|_| BackupError::Invalid)?;
    let known_schema = match doc.version {
        1 => doc.processing.is_none(),
        2 => doc.processing.is_some(),
        _ => false,
    };
    if doc.format.as_str() != "script-hub-relay" || !known_schema {
        return Err(BackupError::Invalid);
    }
    let backup = RelayBackup {
        config: RelayConfig {
            source: std::mem::take(&mut *doc.source),
            source_file: std::mem::take(&mut *doc.source_file),
            output: std::mem::take(&mut *doc.output),
            output_file: std::mem::take(&mut *doc.output_file),
            title: std::mem::take(&mut *doc.title),
            rename: doc.rename,
            sort: doc.sort,
            concurrency: doc.concurrency,
            timeout_seconds: doc.timeout_seconds,
            processing: doc.processing.take().unwrap_or_default(),
            expected_source: Some(doc.content.clone()),
            test_nodes: true,
            ..RelayConfig::default()
        },
        content: std::mem::take(&mut doc.content),
    };
    validate(&backup.config, &backup.content)?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    const CONTENT: &str = "[Proxy]\nA = socks5, node.example.invalid, 1080\n";

    #[test]
    fn dns_plan_roundtrip_is_offline_and_never_restores_consent() {
        let mut config = RelayConfig {
            allow_resolution: true,
            ..Default::default()
        };
        config
            .processing
            .pipeline
            .push(super::super::model::pipeline::Step {
                id: 42,
                enabled: true,
                operation: super::super::model::pipeline::Operation::ResolveDomain {
                    family: super::super::model::pipeline::AddressFamily::PreferV4,
                    options: Default::default(),
                },
            });
        let encoded = encode(&config, CONTENT).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.content.as_str(), CONTENT);
        assert_eq!(decoded.config.processing.pipeline.len(), 1);
        assert!(!decoded.config.allow_resolution);
    }

    #[test]
    fn original_project_backups_restore_without_a_processing_plan() {
        let old = serde_json::json!({
            "format": "script-hub-relay", "version": 1,
            "source": "", "source_file": "", "output": "", "output_file": "nodes.list",
            "title": "Saved relay", "rename": false, "sort": false,
            "concurrency": 4, "timeout_seconds": 8, "content": CONTENT,
        });
        let backup = decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(backup.content.as_str(), CONTENT);
        assert!(!backup.config.processing.is_active());
    }

    #[test]
    fn processing_backup_keeps_the_bound_source_not_the_already_renamed_output() {
        use super::super::model::{
            EnhanceOptions, Health, editor,
            pipeline::{Operation, Step},
        };
        let config = RelayConfig {
            rename: false,
            sort: false,
            expected_source: Some(Zeroizing::new(CONTENT.into())),
            processing: editor::Options {
                pipeline: vec![Step {
                    id: 1,
                    enabled: true,
                    operation: Operation::FormatName {
                        template: "Prefix {name}".into(),
                    },
                }],
                include: String::new(),
                exclude: String::new(),
                find: String::new(),
                replace: String::new(),
                nodes: Vec::new(),
                deduplicate: false,
            },
            ..Default::default()
        };
        let process = |source: &str, options: &editor::Options| {
            let sub = Subscription::parse(source).unwrap();
            sub.render_processed(
                &[Health::Unknown],
                &EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                options,
            )
            .unwrap()
        };
        let output = process(CONTENT, &config.processing);
        let backup = decode(&encode(&config, &output.content).unwrap()).unwrap();
        assert_eq!(backup.content.as_str(), CONTENT);
        assert_eq!(
            process(&backup.content, &backup.config.processing).content,
            output.content
        );
        assert_eq!(backup.config.processing.pipeline.len(), 1);
        assert_eq!(
            backup.config.expected_source.as_deref().map(|s| s.as_str()),
            Some(CONTENT)
        );
    }

    #[test]
    fn supported_subscription_formats_survive_backups_exactly() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let uri = "trojan://synthetic@node.invalid:443#A";
        for content in [uri.to_owned(), STANDARD.encode(uri),
            "proxies:\n - {name: A, type: http, server: node.invalid, port: 80}\n".into(),
            r#"{"outbounds":[{"tag":"A","type":"http","server":"node.invalid","server_port":80}],"endpoints":[]}"#.into()] {
            let bytes = encode(&RelayConfig::default(), &content).unwrap();
            let backup = decode(&bytes).unwrap();
            assert_eq!(&*backup.content, &content);
        }
    }

    #[test]
    fn roundtrip_excludes_security_settings() {
        let config = RelayConfig {
            source: "0123456789abcdef0123456789abcdef".into(),
            source_file: "nodes.list".into(),
            engine: "private-engine".into(),
            compatibility: true,
            doh: true,
            allow_public_output: true,
            allow_resolution: true,
            ..RelayConfig::default()
        };
        let bytes = encode(&config, CONTENT).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        for forbidden in [
            "token",
            "engine",
            "compatibility",
            "doh",
            "allow_public_output",
            "allow_resolution",
            "test_nodes",
        ] {
            assert!(!text.contains(forbidden));
        }
        let backup = decode(&bytes).unwrap();
        assert_eq!(&*backup.content, CONTENT);
        assert_eq!(backup.config.source, config.source);
        assert!(backup.config.test_nodes);
        assert_eq!(backup.config.engine, std::path::PathBuf::from("sing-box"));
        assert!(
            !backup.config.compatibility
                && !backup.config.doh
                && !backup.config.allow_public_output
                && !backup.config.allow_resolution
        );
    }

    #[test]
    fn rejects_malformed_version_unknown_duplicate_and_limits() {
        let bytes = encode(&RelayConfig::default(), CONTENT).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        for bad in [
            "{}".to_owned(),
            s.replace("\"version\":2", "\"version\":1"),
            s.replacen('{', "{\"extra\":1,", 1),
            s.replacen('{', "{\"version\":2,", 1),
        ] {
            assert!(decode(bad.as_bytes()).is_err());
        }
        assert!(matches!(
            decode(&vec![b' '; MAX_FILE + 1]),
            Err(BackupError::TooLarge)
        ));
        assert!(matches!(
            encode(&RelayConfig::default(), &"a".repeat(MAX_CONTENT + 1)),
            Err(BackupError::TooLarge)
        ));
    }

    #[test]
    fn rejects_invalid_content_and_unsafe_configuration() {
        assert!(encode(&RelayConfig::default(), "not a subscription").is_err());
        let config = RelayConfig {
            concurrency: 0,
            ..RelayConfig::default()
        };
        assert!(encode(&config, CONTENT).is_err());
        let config = RelayConfig {
            source: "0123456789abcdef0123456789abcdef".into(),
            output: "0123456789abcdef0123456789abcdef".into(),
            ..RelayConfig::default()
        };
        assert!(encode(&config, CONTENT).is_err());
    }
}
