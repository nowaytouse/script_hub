use std::path::Path;
use std::process::ExitCode;

fn validate_tree(root: &Path) -> Result<(usize, usize), String> {
    let mut artifacts = 0;
    let mut errors = 0;
    let modules_root = root.join("modules");
    let legacy_promax = modules_root.join(
        "surge/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
    );
    if !legacy_promax.is_file() {
        eprintln!(
            "[ERROR] missing legacy PROMAX publication: {}",
            legacy_promax.display()
        );
        errors += 1;
    }
    for directory in [
        modules_root.join("surge"),
        modules_root.join("shadowrocket"),
        modules_root.join("loon"),
    ] {
        if !directory.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(directory)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if matches!(extension, "sgmodule" | "module" | "plugin") {
                let content = match std::fs::read_to_string(path) {
                    Ok(content) => content,
                    Err(error) => {
                        eprintln!("[ERROR] {}: {error}", path.display());
                        errors += 1;
                        continue;
                    }
                };
                artifacts += 1;
                let validation = if extension == "plugin" {
                    rust_processor::promax::validation::validate_loon_plugin(&content)
                } else {
                    rust_processor::promax::validation::validate_surge_module(&content)
                };
                let mut validation = validation;
                validation.extend(
                    rust_processor::promax::validation::validate_apple_compatibility(
                        &content, false,
                    ),
                );
                if !validation.is_empty() {
                    eprintln!(
                        "[ERROR] {}: {} validation error(s)",
                        path.display(),
                        validation.len()
                    );
                    for item in validation.iter().take(3) {
                        eprintln!("        line {} {}", item.line, item.code);
                    }
                    errors += validation.len();
                }
            }
        }
    }

    for directory in [
        root.join("rulesets/AdBlock"),
        root.join("rulesets/RULE-SET"),
    ] {
        if !directory.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&directory)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("list")
            {
                continue;
            }
            let content = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(error) => {
                    eprintln!("[ERROR] {}: {error}", path.display());
                    errors += 1;
                    continue;
                }
            };
            artifacts += 1;
            let validation =
                rust_processor::promax::validation::validate_external_ruleset(&content);
            let mut validation = validation;
            validation.extend(
                rust_processor::promax::validation::validate_apple_compatibility(
                    &content,
                    directory.ends_with("AdBlock"),
                ),
            );
            if !validation.is_empty() {
                eprintln!(
                    "[ERROR] {}: {} validation error(s)",
                    path.display(),
                    validation.len()
                );
                for item in validation.iter().take(3) {
                    eprintln!("        line {} {}", item.line, item.code);
                }
                errors += validation.len();
            }
        }
    }

    let port_source = root.join("rulesets/Sources/conf/SurgeConf_DirectPorts.list");
    if port_source.is_file() {
        let content = std::fs::read_to_string(&port_source).map_err(|error| error.to_string())?;
        let validation =
            rust_processor::promax::validation::validate_apple_compatibility(&content, false);
        artifacts += 1;
        if !validation.is_empty() {
            eprintln!(
                "[ERROR] {}: {} Apple compatibility error(s)",
                port_source.display(),
                validation.len()
            );
            for item in validation.iter().take(3) {
                eprintln!("        line {} {}", item.line, item.code);
            }
            errors += validation.len();
        }
    }

    artifacts += 1;
    let boundary_errors = rust_processor::general_update::validate_category_boundaries(root)?;
    for message in &boundary_errors {
        eprintln!("[ERROR] ruleset category contamination: {message}");
    }
    errors += boundary_errors.len();
    Ok((artifacts, errors))
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let root = match (args.next().as_deref(), args.next()) {
        (Some("--root"), Some(root)) => root,
        _ => {
            eprintln!("Usage: validate --root <repository>");
            return ExitCode::from(2);
        }
    };
    match validate_tree(Path::new(&root)) {
        Ok((artifacts, 0)) => {
            println!("[SUCCESS] validated {artifacts} published artifacts");
            ExitCode::SUCCESS
        }
        Ok((artifacts, errors)) => {
            eprintln!(
                "[ERROR] {artifacts} published artifacts contain {errors} validation error(s)"
            );
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("[ERROR] {error}");
            ExitCode::from(1)
        }
    }
}
