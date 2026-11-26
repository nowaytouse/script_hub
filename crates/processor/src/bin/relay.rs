use rust_processor::relay::{RelayConfig, RunMode, profile};
use rust_processor::vault;
use std::io::Read;
use std::path::{self, PathBuf};
use std::{process::ExitCode, sync::atomic::AtomicBool};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const MAX_REQUEST_BYTES: u64 = 64 * 1024;

#[derive(serde::Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct ProfileRequest {
    template: String,
    nodes: String,
    group: String,
    output: String,
}

fn read_request_bytes(reader: impl Read) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read relay configuration from standard input.")?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err("Relay configuration exceeds 64 KiB.");
    }
    Ok(bytes)
}

fn read_config(reader: impl Read) -> Result<RelayConfig, &'static str> {
    let bytes = read_request_bytes(reader)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "Invalid relay configuration; use known settings only, without a token field.")
}

fn read_profile_request(reader: impl Read) -> Result<ProfileRequest, &'static str> {
    let bytes = read_request_bytes(reader)?;
    serde_json::from_slice(&bytes).map_err(
        |_| "Invalid Mihomo profile request; specify template, nodes, group, and output only.",
    )
}

fn absolute_profile_path(value: &str) -> Result<PathBuf, &'static str> {
    let path = path::absolute(value).map_err(|_| "Invalid profile file path.")?;
    #[cfg(windows)]
    if !matches!(
        path.components().next(),
        Some(path::Component::Prefix(prefix))
            if matches!(prefix.kind(), path::Prefix::Disk(_) | path::Prefix::VerbatimDisk(_))
    ) {
        return Err("Profile files must use local drive paths.");
    }
    Ok(path)
}

fn read_profile_file(value: &str) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let path = absolute_profile_path(value)?;
    vault::read_bounded(&path)
        .map(Zeroizing::new)
        .map_err(|error| match error {
            vault::VaultError::TooLarge => "A profile input exceeds the supported file size.",
            _ => "Could not safely read a profile input file.",
        })
}

fn build_mihomo_profile(reader: impl Read) -> Result<(), String> {
    let request = read_profile_request(reader)?;
    let output = absolute_profile_path(&request.output)?;
    let template = {
        let bytes = read_profile_file(&request.template)?;
        let yaml = std::str::from_utf8(&bytes).map_err(|_| "The template must be UTF-8.")?;
        profile::MihomoTemplate::parse(yaml).map_err(|error| error.to_string())?
    };
    let bytes = read_profile_file(&request.nodes)?;
    let nodes = std::str::from_utf8(&bytes).map_err(|_| "The node list must be UTF-8.")?;
    let yaml = template
        .build(nodes, &request.group)
        .map_err(|error| error.to_string())?;
    profile::write_private_yaml(&output, &yaml)
}

fn main() -> ExitCode {
    if rust_processor::app::print_version("script-hub-relay") {
        return ExitCode::SUCCESS;
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.as_slice() == ["--help"] {
        println!(
            "script-hub-relay [--publish | --build-mihomo-profile]\nRead RelayConfig JSON from stdin. Preview is the default; GIST_TOKEN is session-only.\nRequired: source; source_file unless explicit in raw URL.\nPublish requires output_file and GIST_TOKEN; blank output creates one secret Gist, otherwise a distinct existing output is reused.\nFormatting does not need sing-box. Set test_nodes:true and configure engine for protocol-aware HTTPS checks. Source certificate bypass is first tested with strict verification unless compatibility is explicitly enabled. Untested nodes are not marked healthy or removed.\nOnly counts, non-sensitive check details and a created Gist ID/URL are printed; use the desktop node panel to edit or copy nodes. Save the created target for later runs. Never retry an uncertain create without checking your Gists.\n\n--build-mihomo-profile reads a separate JSON request from stdin (up to 64 KiB).\nSet template to a local Mihomo YAML path, nodes to a local node-list path, group to the target proxy group name, and output to a new local .yaml or .yml path.\nThis mode builds offline and refuses to replace an existing file."
        );
        return ExitCode::SUCCESS;
    }
    if args.as_slice() == ["--build-mihomo-profile"] {
        return match build_mihomo_profile(std::io::stdin().lock()) {
            Ok(()) => {
                println!("{{\"written\":true}}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    let mode = match args.as_slice() {
        [] => RunMode::Preview,
        [arg] if arg == "--publish" => RunMode::Publish,
        _ => {
            eprintln!(
                "Use --help; private configuration belongs on stdin, not command-line arguments."
            );
            return ExitCode::FAILURE;
        }
    };
    let config = match read_config(std::io::stdin().lock()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let token = Zeroizing::new(match std::env::var("GIST_TOKEN") {
        Ok(token) => token,
        Err(std::env::VarError::NotPresent) => String::new(),
        Err(_) => {
            eprintln!("Invalid session token encoding.");
            return ExitCode::FAILURE;
        }
    });
    match rust_processor::relay::run(&config, &token, mode, &AtomicBool::new(false), &|_| {}) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::json!({
                    "tested": result.tested,
                    "total": result.total,
                    "processed": result.processed,
                    "excluded": result.excluded,
                    "duplicates": result.duplicates,
                    "duplicate_skipped": result.duplicate_skipped,
                    "healthy": result.healthy,
                    "failed": result.failed,
                    "unknown": result.unknown,
                    "security_blocked": result.security_blocked,
                    "removed": result.removed,
                    "baseline_valid": result.baseline_valid,
                    "details": result.details,
                    "published": result.published,
                    "unchanged": result.unchanged,
                    "created_gist_id": result.created.as_ref().map(|created| &created.id),
                    "created_gist_url": result.created.as_ref().map(|created| &created.url)
                })
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_stdin_accepts_only_the_known_config_shape() {
        let config =
            read_config(br#"{"source":"0123456789abcdef","source_file":"nodes.txt"}"#.as_slice())
                .unwrap();
        assert_eq!(config.concurrency, 4);
        assert!(read_config(br#"{"token":"never accept secrets here"}"#.as_slice()).is_err());
        assert!(read_config(vec![b' '; 65537].as_slice()).is_err());
        assert!(read_config([0xff].as_slice()).is_err());
    }

    #[test]
    fn bounded_profile_request_uses_only_the_four_documented_fields() {
        let json = br#"{"template":"base.yaml","nodes":"nodes.list","group":"Select","output":"new.yaml"}"#;
        let request = read_profile_request(json.as_slice()).unwrap();
        assert_eq!(request.template, "base.yaml");
        assert_eq!(request.nodes, "nodes.list");
        assert_eq!(request.group, "Select");
        assert_eq!(request.output, "new.yaml");
        let unknown = br#"{"template":"base.yaml","nodes":"nodes.list","group":"Select","output":"new.yaml","secret-example":"private-key"}"#;
        assert!(matches!(
            read_profile_request(unknown.as_slice()),
            Err("Invalid Mihomo profile request; specify template, nodes, group, and output only.")
        ));
        assert!(read_profile_request(br#"{"template":"base.yaml"}"#.as_slice()).is_err());
        assert!(
            read_profile_request(vec![b' '; MAX_REQUEST_BYTES as usize + 1].as_slice()).is_err()
        );
    }
}
