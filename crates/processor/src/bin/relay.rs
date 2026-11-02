use rust_processor::relay::{RelayConfig, RunMode};
use std::io::Read;
use std::{process::ExitCode, sync::atomic::AtomicBool};
use zeroize::Zeroizing;

fn read_config(reader: impl Read) -> Result<RelayConfig, &'static str> {
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read relay configuration from standard input.")?;
    if bytes.len() > 65536 {
        return Err("Relay configuration exceeds 64 KiB.");
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "Invalid relay configuration; use known settings only, without a token field.")
}

fn main() -> ExitCode {
    if rust_processor::app::print_version("script-hub-relay") {
        return ExitCode::SUCCESS;
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.as_slice() == ["--help"] {
        println!(
            "script-hub-relay [--publish]\nRead RelayConfig JSON from stdin. Preview is the default; GIST_TOKEN is session-only.\nRequired: source; source_file unless explicit in raw URL.\nPublish requires output_file and GIST_TOKEN; blank output creates one secret Gist, otherwise a distinct existing output is reused.\nFormatting does not need sing-box. Set test_nodes:true and configure engine for protocol-aware HTTPS checks. Source certificate bypass is first tested with strict verification unless compatibility is explicitly enabled. Untested nodes are not marked healthy or removed.\nOnly counts, non-sensitive check details and a created Gist ID/URL are printed; use the desktop node panel to edit or copy nodes. Save the created target for later runs. Never retry an uncertain create without checking your Gists."
        );
        return ExitCode::SUCCESS;
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
}
