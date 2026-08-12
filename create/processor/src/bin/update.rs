use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::ExitCode;

fn discard_semantic_noop_changes(root: &Path) -> Result<(), String> {
    let names = Command::new("git")
        .args(["diff", "--name-only", "--", "modules", "rulesets", "dns"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot inspect generated changes: {error}"))?;
    if !names.status.success() {
        return Err("cannot inspect generated changes".to_string());
    }
    for name in String::from_utf8_lossy(&names.stdout)
        .lines()
        .filter(|line| !line.is_empty())
    {
        let path = root.join(name);
        if !path.is_file() {
            continue;
        }
        let old = Command::new("git")
            .args(["show", &format!("HEAD:{name}")])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot read HEAD:{name}: {error}"))?;
        if !old.status.success() {
            continue;
        }
        let old_text = String::from_utf8_lossy(&old.stdout);
        let new_text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read generated file {name}: {error}"))?;
        if rust_processor::url_rewriter::semantic_content_for_path_pub(&path, &old_text)
            == rust_processor::url_rewriter::semantic_content_for_path_pub(&path, &new_text)
        {
            std::fs::write(&path, old.stdout)
                .map_err(|error| format!("cannot restore semantic no-op {name}: {error}"))?;
            println!("[INFO] Restored semantic no-op: {name}");
        }
    }
    Ok(())
}

fn arg_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn run(root: &Path, execute: bool, singbox: Option<&Path>) -> Result<(), String> {
    if !root
        .join("rulesets/Sources/adblock_whitelist.list")
        .is_file()
    {
        return Err(format!(
            "repository root is missing PROMAX whitelist: {}",
            root.display()
        ));
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

    let cleanup = rust_processor::smart_cleanup::run_cleanup(&root.to_string_lossy());
    println!(
        "[INFO] ruleset cleanup deleted {} file(s)",
        cleanup.get("deleted").copied().unwrap_or_default()
    );

    let modules = root.join("modules");
    let rulesets = root.join("rulesets");
    rust_processor::url_rewriter::copy_github_variants(&root.to_string_lossy());
    for directory in [&modules, &rulesets, root] {
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

    let ports_source = root.join("rulesets/Sources/conf/SurgeConf_DirectPorts.list");
    let firewall_modules = [
        modules.join("surge/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"),
        modules.join("shadowrocket/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.module"),
    ];
    let firewall_refs: Vec<&str> = firewall_modules
        .iter()
        .filter_map(|path| path.to_str())
        .collect();
    if ports_source.is_file() {
        rust_processor::firewall_sync::sync_ports(
            &ports_source.to_string_lossy(),
            &firewall_refs,
            execute,
            &chrono::Local::now().format("%Y.%m.%d").to_string(),
        );
    }

    if let Some(singbox) = singbox.filter(|path| path.is_file()) {
        if !rust_processor::srs_generator::run_srs_generator(
            &root.to_string_lossy(),
            &singbox.to_string_lossy(),
        ) {
            return Err("SRS generation failed".to_string());
        }
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

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut singbox = None;
    let mut execute = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = Some(arg_value(&mut args, "--root")),
            "--singbox" => singbox = Some(arg_value(&mut args, "--singbox")),
            "--execute" | "--unattended" => execute = true,
            "--quick" => {}
            "--help" | "-h" => {
                println!(
                    "Usage: update --root <repository> [--execute|--unattended] [--singbox <path>]"
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

    match run(&root, execute, singbox.as_deref()) {
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
