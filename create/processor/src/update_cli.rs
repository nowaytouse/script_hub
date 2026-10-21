use crate as rust_processor;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

fn read_batch_blob(
    input: &mut impl Write,
    output: &mut impl BufRead,
    object: &str,
) -> Result<Option<Vec<u8>>, String> {
    writeln!(input, "{object}").map_err(|error| format!("cannot query git object: {error}"))?;
    input
        .flush()
        .map_err(|error| format!("cannot query git object: {error}"))?;

    let mut header = String::new();
    if output
        .read_line(&mut header)
        .map_err(|error| format!("cannot read git object header: {error}"))?
        == 0
    {
        return Err(format!("git returned no object header for {object}"));
    }
    let header = header.trim_end_matches(['\r', '\n']);
    if header.ends_with(" missing") {
        return Ok(None);
    }
    let (metadata, size) = header
        .rsplit_once(' ')
        .ok_or_else(|| format!("invalid git object header for {object}"))?;
    let (_, kind) = metadata
        .rsplit_once(' ')
        .ok_or_else(|| format!("invalid git object header for {object}"))?;
    if kind != "blob" {
        return Err(format!("expected a git blob for {object}, found {kind}"));
    }
    let size = size
        .parse::<usize>()
        .map_err(|error| format!("invalid git object size for {object}: {error}"))?;
    let mut content = vec![0; size];
    output
        .read_exact(&mut content)
        .map_err(|error| format!("cannot read git object {object}: {error}"))?;
    let mut separator = [0];
    output
        .read_exact(&mut separator)
        .map_err(|error| format!("cannot read git object separator: {error}"))?;
    if separator[0] != b'\n' {
        return Err(format!("invalid git object separator for {object}"));
    }
    Ok(Some(content))
}

fn read_head_blob(
    root: &Path,
    name: &str,
    input: &mut impl Write,
    output: &mut impl BufRead,
) -> Result<Option<Vec<u8>>, String> {
    if name.contains('\n') || name.contains('\r') {
        let old = Command::new("git")
            .args(["show", &format!("HEAD:{name}")])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot read HEAD:{name}: {error}"))?;
        return Ok(old.status.success().then_some(old.stdout));
    }
    read_batch_blob(input, output, &format!("HEAD:{name}"))
}

fn discard_semantic_noop_changes(root: &Path) -> Result<(), String> {
    let names = Command::new("git")
        .args([
            "diff",
            "--name-only",
            "-z",
            "--",
            "modules",
            "rulesets",
            "dns",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot inspect generated changes: {error}"))?;
    if !names.status.success() {
        return Err("cannot inspect generated changes".to_string());
    }
    let changed_names = String::from_utf8_lossy(&names.stdout);
    let files: Vec<(String, PathBuf)> = changed_names
        .split('\0')
        .filter(|line| !line.is_empty())
        .filter_map(|name| {
            let path = root.join(name);
            let source_input = rust_processor::url_rewriter::is_pipeline_source_path_pub(&path)
                && !name.starts_with("modules/source/build/");
            (path.is_file() && !source_input).then(|| (name.to_string(), path))
        })
        .collect();
    if files.is_empty() {
        return Ok(());
    }

    let mut cat_file = Command::new("git")
        .args(["cat-file", "--batch"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot inspect generated blobs: {error}"))?;
    let mut input = BufWriter::new(
        cat_file
            .stdin
            .take()
            .ok_or_else(|| "cannot open git object input".to_string())?,
    );
    let mut output = BufReader::new(
        cat_file
            .stdout
            .take()
            .ok_or_else(|| "cannot open git object output".to_string())?,
    );
    let processing = (|| {
        for (name, path) in files {
            let Some(old_bytes) = read_head_blob(root, &name, &mut input, &mut output)? else {
                continue;
            };
            let Ok(new_bytes) = std::fs::read(&path) else {
                return Err(format!("cannot read generated file {name}"));
            };
            if old_bytes.contains(&0) || new_bytes.contains(&0) {
                continue;
            }
            let old_text = String::from_utf8_lossy(&old_bytes);
            let new_text = String::from_utf8_lossy(&new_bytes);
            if rust_processor::url_rewriter::semantic_content_for_path_pub(&path, &old_text)
                == rust_processor::url_rewriter::semantic_content_for_path_pub(&path, &new_text)
            {
                std::fs::write(&path, old_bytes)
                    .map_err(|error| format!("cannot restore semantic no-op {name}: {error}"))?;
                println!("[INFO] Restored semantic no-op: {name}");
            }
        }
        Ok(())
    })();
    drop(input);
    drop(output);
    let status = cat_file
        .wait()
        .map_err(|error| format!("cannot finish git object read: {error}"))?;
    processing?;
    if !status.success() {
        return Err("cannot read generated blobs from git".to_string());
    }
    Ok(())
}

fn arg_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn run(root: &Path, execute: bool, singbox: Option<&Path>, quick: bool) -> Result<(), String> {
    if !root
        .join("rulesets/Sources/adblock_whitelist.list")
        .is_file()
    {
        return Err(format!(
            "repository root is missing PROMAX whitelist: {}",
            root.display()
        ));
    }

    if execute && !quick {
        let failures = rust_processor::functional_update::run_functional_updates(root);
        if !failures.is_empty() {
            eprintln!(
                "[WARN] {} functional bundle(s) kept at their last validated version: {}",
                failures.len(),
                failures.join(", ")
            );
        }
        let failures = rust_processor::general_update::run_general_updates(root);
        if !failures.is_empty() {
            eprintln!(
                "[WARN] {} general ruleset(s) kept at their last validated version: {}",
                failures.len(),
                failures.join(", ")
            );
        }
    }

    // PROMAX owns both source downloads and local compilation.  Keep the rest
    // of the Rust post-processing in one deterministic, low-concurrency pass.
    if !rust_processor::promax::run_adblock_manager(&root.to_string_lossy(), execute) {
        return Err("PROMAX compilation failed".to_string());
    }

    if !execute {
        println!("[INFO] dry run: generation and post-processing skipped");
        return Ok(());
    }

    let cleanup = rust_processor::smart_cleanup::run_cleanup(&root.to_string_lossy())?;
    println!(
        "[INFO] ruleset cleanup deleted {} file(s)",
        cleanup.get("deleted").copied().unwrap_or_default()
    );
    rust_processor::general_update::enforce_category_boundaries(root)?;
    rust_processor::general_update::split_gaming_rules(root)?;

    let modules = root.join("modules");
    let rulesets = root.join("rulesets");
    for directory in [&modules, &rulesets] {
        let changed = rust_processor::url_rewriter::run_url_rewrites(&directory.to_string_lossy());
        if changed < 0 {
            return Err(format!("URL rewrite failed for {}", directory.display()));
        }
    }

    let surge = modules.join("surge");
    if surge.is_dir()
        && rust_processor::mitm_manager::run_mitm_cleanup(&surge.to_string_lossy(), false) < 0
    {
        return Err("MITM cleanup failed".to_string());
    }
    let converted = rust_processor::functional_update::sync_shadowrocket_variants(root)?;
    println!("[INFO] synchronized {converted} Shadowrocket module variant(s)");

    let publication_version = rust_processor::publication_now()
        .format("%Y.%m.%d")
        .to_string();
    let refreshed =
        rust_processor::firewall_sync::refresh_security_modules(root, &publication_version)?;
    println!("[INFO] refreshed {refreshed} DNS/Apple/firewall helper module(s)");

    let ports_source = root.join("rulesets/Sources/conf/SurgeConf_DirectPorts.list");
    let firewall_modules = [
        modules.join("surge/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"),
        modules.join("shadowrocket/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.module"),
        modules.join("shadowrocket/head_expanse/🛡️ PROMAX AdBlock Helper (DNS & Firewall).module"),
    ];
    let firewall_refs: Vec<&str> = firewall_modules
        .iter()
        .filter(|path| path.is_file())
        .filter_map(|path| path.to_str())
        .collect();
    if ports_source.is_file() {
        rust_processor::firewall_sync::sync_ports(
            &ports_source.to_string_lossy(),
            &firewall_refs,
            execute,
            &publication_version,
        );
    }
    rust_processor::url_rewriter::copy_github_variants(&root.to_string_lossy())?;
    rust_processor::module_catalog::refresh(root)?;

    if let Some(singbox) = singbox.filter(|path| path.is_file())
        && !rust_processor::srs_generator::run_srs_generator(
            &root.to_string_lossy(),
            &singbox.to_string_lossy(),
        )
    {
        return Err("SRS generation failed".to_string());
    }

    if execute {
        discard_semantic_noop_changes(root)?;
        let status = Command::new("git")
            .args(["diff", "--check"])
            .current_dir(root)
            .status()
            .map_err(|error| format!("git diff check failed to start: {error}"))?;
        if !status.success() {
            return Err("generated tree contains whitespace errors".to_string());
        }
    }

    Ok(())
}

pub fn run_cli(mut args: impl Iterator<Item = String>) -> ExitCode {
    let mut root = None;
    let mut singbox = None;
    let mut execute = false;
    let mut quick = false;
    let mut gaming_only = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = Some(arg_value(&mut args, "--root")),
            "--singbox" => singbox = Some(arg_value(&mut args, "--singbox")),
            "--execute" | "--unattended" => execute = true,
            "--quick" => quick = true,
            "--gaming-only" => gaming_only = true,
            "--help" | "-h" => {
                println!(
                    "Usage: update --root <repository> [--execute|--unattended] [--singbox <path>] [--gaming-only]"
                );
                return ExitCode::SUCCESS;
            }
            unknown => {
                eprintln!("unknown argument: {unknown}");
                return ExitCode::from(2);
            }
        }
    }

    let root = match root {
        Some(Ok(path)) => path,
        Some(Err(error)) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
        None => {
            eprintln!("missing required --root <repository> argument");
            return ExitCode::from(2);
        }
    };
    let singbox = match singbox {
        Some(Ok(path)) => Some(path),
        Some(Err(error)) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
        None => None,
    };

    let result = if gaming_only && !execute {
        println!("[INFO] dry run: game classification skipped");
        Ok(())
    } else if gaming_only {
        rust_processor::general_update::split_gaming_rules(&root).and_then(|()| {
            if let Some(singbox) = &singbox
                && !rust_processor::srs_generator::run_srs_generator(
                    &root.to_string_lossy(),
                    &singbox.to_string_lossy(),
                )
            {
                return Err("SRS generation failed".to_string());
            }
            Ok(())
        })
    } else {
        run(&root, execute, singbox.as_deref(), quick)
    };
    match result {
        Ok(()) => {
            println!("[SUCCESS] Rust ruleset update completed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("[ERROR] {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{discard_semantic_noop_changes, read_batch_blob};
    use std::io::Cursor;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reads_sequential_git_batch_responses_without_losing_blob_boundaries() {
        let mut output = Cursor::new(
            b"oid-1 blob 4\none\n\noid-2 blob 9\ntwo\0three\nHEAD:missing-path missing\n".to_vec(),
        );
        let mut input = Vec::new();

        assert_eq!(
            read_batch_blob(&mut input, &mut output, "oid-1")
                .unwrap()
                .unwrap(),
            b"one\n"
        );
        assert_eq!(
            read_batch_blob(&mut input, &mut output, "oid-2")
                .unwrap()
                .unwrap(),
            b"two\0three"
        );
        assert!(
            read_batch_blob(&mut input, &mut output, "missing")
                .unwrap()
                .is_none()
        );
        assert_eq!(input, b"oid-1\noid-2\nmissing\n");
    }

    #[test]
    fn rejects_malformed_git_object_headers() {
        let mut output = Cursor::new(b"malformed\n".to_vec());

        assert_eq!(
            read_batch_blob(&mut Vec::new(), &mut output, "oid").unwrap_err(),
            "invalid git object header for oid"
        );
    }

    #[test]
    fn semantic_noop_filter_restores_a_tracked_file_with_spaces_in_its_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("script-hub-noop-{}-{unique}", std::process::id()));
        let relative = "rulesets/RULE-SET/Direct routing.list";
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = b"# baseline\nDOMAIN-SUFFIX,example.test\n";
        std::fs::write(&path, original).unwrap();

        let git = |arguments: &[&str]| {
            let output = Command::new("git")
                .args(arguments)
                .current_dir(&root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {arguments:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "--quiet"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "Script Hub Test"]);
        git(&["add", "--", relative]);
        git(&["commit", "--quiet", "-m", "baseline"]);
        std::fs::write(&path, b"# updated date\nDOMAIN-SUFFIX,example.test\n").unwrap();

        discard_semantic_noop_changes(&root).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), original);
        let _ = std::fs::remove_dir_all(root);
    }
}
