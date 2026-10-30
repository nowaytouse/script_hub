use crate as rust_processor;
use crate::desktop::progress::{Stage, emit_stage};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const MAX_GENERATED_FILE_BYTES: usize = 128 * 1024 * 1024;

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
    if size > MAX_GENERATED_FILE_BYTES {
        return Err(format!(
            "git object {object} exceeds the generated-file byte limit"
        ));
    }
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
            .args([
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("HEAD:{name}"),
            ])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot read HEAD:{name}: {error}"))?;
        if !old.status.success() {
            return Ok(None);
        }
        let object = String::from_utf8(old.stdout)
            .map_err(|error| format!("invalid git object ID: {error}"))?;
        return read_batch_blob(input, output, object.trim());
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
    let mut files = Vec::new();
    for name in changed_names.split('\0').filter(|name| !name.is_empty()) {
        let path = root.join(name);
        let source_input = rust_processor::url_rewriter::is_pipeline_source_path_pub(&path)
            && !name.starts_with("modules/source/build/")
            && !name.starts_with("rulesets/Sources/compiled/");
        if source_input {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("cannot inspect generated file {name}: {error}")),
        };
        if metadata.file_type().is_symlink() {
            return Err(format!("generated file is a symlink: {name}"));
        }
        if !metadata.is_file() {
            continue;
        }
        for parent in path
            .ancestors()
            .skip(1)
            .take_while(|parent| *parent != root)
        {
            let metadata = std::fs::symlink_metadata(parent)
                .map_err(|error| format!("cannot inspect generated parent: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "generated parent is a symlink: {}",
                    parent.display()
                ));
            }
        }
        files.push((name.to_string(), path));
    }
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
            let mut new_bytes = Vec::new();
            std::fs::File::open(&path)
                .and_then(|file| {
                    file.take(MAX_GENERATED_FILE_BYTES as u64 + 1)
                        .read_to_end(&mut new_bytes)
                })
                .map_err(|error| format!("cannot read generated file {name}: {error}"))?;
            if new_bytes.len() > MAX_GENERATED_FILE_BYTES {
                return Err(format!("generated file {name} exceeds the byte limit"));
            }
            if old_bytes.contains(&0) || new_bytes.contains(&0) {
                continue;
            }
            let old_text = String::from_utf8_lossy(&old_bytes);
            let new_text = String::from_utf8_lossy(&new_bytes);
            if rust_processor::url_rewriter::semantic_content_for_path_pub(&path, &old_text)
                == rust_processor::url_rewriter::semantic_content_for_path_pub(&path, &new_text)
            {
                crate::filesystem::replace_file(&path, &old_bytes)
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

fn run(
    root: &Path,
    execute: bool,
    singbox: Option<&Path>,
    quick: bool,
    adblock_only: bool,
) -> Result<(), String> {
    emit_stage(Stage::Prepare);
    if !root
        .join("rulesets/Sources/adblock_whitelist.list")
        .is_file()
    {
        return Err(format!(
            "repository root is missing PROMAX whitelist: {}",
            root.display()
        ));
    }

    if execute && !quick && !adblock_only {
        emit_stage(Stage::Functional);
        let failures = rust_processor::functional_update::run_functional_updates(root);
        if !failures.is_empty() {
            eprintln!(
                "[WARN] {} functional bundle(s) kept at their last validated version: {}",
                failures.len(),
                failures.join(", ")
            );
        }
        emit_stage(Stage::General);
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
    emit_stage(Stage::Adblock);
    if !rust_processor::promax::run_adblock_manager(&root.to_string_lossy(), execute) {
        return Err("PROMAX compilation failed".to_string());
    }

    if !execute {
        println!("[INFO] dry run: generation and post-processing skipped");
        return Ok(());
    }

    emit_stage(Stage::Cleanup);
    rust_processor::general_update::prepare_ruleset_inputs(root)?;
    let cleanup = rust_processor::smart_cleanup::run_cleanup(&root.to_string_lossy())?;
    println!(
        "[INFO] ruleset cleanup deleted {} file(s)",
        cleanup.get("deleted").copied().unwrap_or_default()
    );
    emit_stage(Stage::Categories);
    rust_processor::general_update::enforce_category_boundaries(root)?;
    emit_stage(Stage::Gaming);
    rust_processor::general_update::split_gaming_rules(root)?;
    rust_processor::general_update::consolidate_rulesets(root)?;
    rust_processor::general_update::enforce_jp_ownership(root)?;
    rust_processor::general_update::split_published_rulesets(root)?;
    let retired = rust_processor::general_update::retire_ruleset_inputs(root)?;
    println!("[INFO] retired {retired} superseded ruleset artifacts; current subscriptions only");

    let modules = root.join("modules");
    let rulesets = root.join("rulesets");
    emit_stage(Stage::Rewrites);
    for directory in [&modules, &rulesets] {
        let changed = rust_processor::url_rewriter::run_url_rewrites(&directory.to_string_lossy());
        if changed < 0 {
            return Err(format!("URL rewrite failed for {}", directory.display()));
        }
    }

    let surge = modules.join("surge");
    emit_stage(Stage::Mitm);
    if surge.is_dir()
        && rust_processor::mitm_manager::run_mitm_cleanup(&surge.to_string_lossy(), false) < 0
    {
        return Err("MITM cleanup failed".to_string());
    }
    emit_stage(Stage::Shadowrocket);
    let converted = rust_processor::functional_update::sync_shadowrocket_variants(root)?;
    println!("[INFO] synchronized {converted} Shadowrocket module variant(s)");

    emit_stage(Stage::Security);
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
    emit_stage(Stage::Mirrors);
    rust_processor::url_rewriter::copy_github_variants(&root.to_string_lossy())?;
    if singbox.is_some_and(|path| path.is_file()) {
        emit_stage(Stage::Srs);
    }
    if let Some(singbox) = singbox.filter(|path| path.is_file())
        && !rust_processor::srs_generator::run_srs_generator(
            &root.to_string_lossy(),
            &singbox.to_string_lossy(),
        )
    {
        return Err("SRS generation failed".to_string());
    }

    if execute {
        emit_stage(Stage::Finalize);
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
    let mut adblock_only = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = Some(arg_value(&mut args, "--root")),
            "--singbox" => singbox = Some(arg_value(&mut args, "--singbox")),
            "--execute" | "--unattended" => execute = true,
            "--quick" => quick = true,
            "--gaming-only" => gaming_only = true,
            "--adblock-only" => adblock_only = true,
            "--help" | "-h" => {
                println!(
                    "Usage: update --root <repository> [--execute|--unattended] [--singbox <path>] [--quick|--gaming-only|--adblock-only]"
                );
                return ExitCode::SUCCESS;
            }
            unknown => {
                eprintln!("unknown argument: {unknown}");
                return ExitCode::from(2);
            }
        }
    }

    if adblock_only && (quick || gaming_only) {
        eprintln!("--adblock-only cannot be combined with --quick or --gaming-only");
        return ExitCode::from(2);
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
        emit_stage(Stage::Prepare);
        emit_stage(Stage::Gaming);
        rust_processor::general_update::split_gaming_rules(&root).and_then(|()| {
            rust_processor::general_update::split_published_rulesets(&root)?;
            rust_processor::url_rewriter::copy_github_variants(&root.to_string_lossy())?;
            if let Some(singbox) = &singbox {
                emit_stage(Stage::Srs);
                if !rust_processor::srs_generator::run_srs_generator(
                    &root.to_string_lossy(),
                    &singbox.to_string_lossy(),
                ) {
                    return Err("SRS generation failed".to_string());
                }
            }
            Ok(())
        })
    } else {
        run(&root, execute, singbox.as_deref(), quick, adblock_only)
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
    use super::{discard_semantic_noop_changes, read_batch_blob, run_cli};
    use std::io::Cursor;
    use std::process::{Command, ExitCode};
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
    fn rejects_oversized_git_blobs_before_reading_payload() {
        let mut output = Cursor::new(b"oid blob 134217729\n".to_vec());
        assert!(
            read_batch_blob(&mut Vec::new(), &mut output, "oid")
                .unwrap_err()
                .contains("exceeds")
        );
    }

    #[cfg(unix)]
    #[test]
    fn semantic_noop_filter_does_not_write_through_symlinks() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("script-hub-noop-symlink-{unique}"));
        let path = root.join("rulesets/RULE-SET/fixture.list");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"# baseline\nDOMAIN-SUFFIX,example.test\n").unwrap();
        for arguments in [
            vec!["init", "--quiet"],
            vec!["add", "--", "rulesets/RULE-SET/fixture.list"],
            vec![
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "baseline",
            ],
        ] {
            assert!(
                Command::new("git")
                    .args(arguments)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let sentinel = root.join("unrelated.list");
        let contents = b"# keep this metadata\nDOMAIN-SUFFIX,example.test\n";
        std::fs::write(&sentinel, contents).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&sentinel, &path).unwrap();
        let result = discard_semantic_noop_changes(&root);
        assert_eq!(std::fs::read(&sentinel).unwrap(), contents);
        assert!(result.unwrap_err().contains("symlink"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn adblock_only_rejects_other_update_modes() {
        for mode in ["--quick", "--gaming-only"] {
            assert_eq!(
                run_cli(
                    ["--root", ".", "--adblock-only", mode]
                        .into_iter()
                        .map(str::to_string)
                ),
                ExitCode::from(2)
            );
        }
    }

    #[test]
    fn semantic_noop_filter_restores_compiled_outputs_but_not_authored_sources() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "script-hub-compiled-noop-{}-{unique}",
            std::process::id()
        ));
        let compiled = "rulesets/Sources/compiled/Cloudflare_ip.list";
        let local = "rulesets/Sources/Local/fixture.list";
        let links = "rulesets/Sources/Links/fixture.list";
        for relative in [compiled, local, links] {
            std::fs::create_dir_all(root.join(relative).parent().unwrap()).unwrap();
        }
        let compiled_original = b"# generated baseline\nDOMAIN-SUFFIX,example.test\n";
        let local_original = b"# authored baseline\nDOMAIN-SUFFIX,local.example\n";
        let links_original = b"# authored manifest baseline\nfixture.list\n";
        std::fs::write(root.join(compiled), compiled_original).unwrap();
        std::fs::write(root.join(local), local_original).unwrap();
        std::fs::write(root.join(links), links_original).unwrap();

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
        git(&["add", "--", compiled, local, links]);
        git(&["commit", "--quiet", "-m", "baseline"]);

        let compiled_changed = b"# generated refreshed timestamp\nDOMAIN-SUFFIX,example.test\n";
        let local_changed = b"# authored refreshed timestamp\nDOMAIN-SUFFIX,local.example\n";
        let links_changed = b"# authored refreshed manifest\nfixture.list\n";
        std::fs::write(root.join(compiled), compiled_changed).unwrap();
        std::fs::write(root.join(local), local_changed).unwrap();
        std::fs::write(root.join(links), links_changed).unwrap();

        discard_semantic_noop_changes(&root).unwrap();

        assert_eq!(
            std::fs::read(root.join(compiled)).unwrap(),
            compiled_original
        );
        assert_eq!(std::fs::read(root.join(local)).unwrap(), local_changed);
        assert_eq!(std::fs::read(root.join(links)).unwrap(), links_changed);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn semantic_noop_filter_restores_tracked_files_with_whitespace_in_their_paths() {
        for relative in [
            "rulesets/RULE-SET/Direct routing.list",
            "rulesets/RULE-SET/line\nbreak.list",
        ] {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("script-hub-noop-{}-{unique}", std::process::id()));
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
}
