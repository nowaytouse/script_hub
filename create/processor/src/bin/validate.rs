use std::path::Path;
use std::process::ExitCode;

fn validate_tree(root: &Path) -> Result<(usize, usize), String> {
    let mut artifacts = 0;
    let mut errors = 0;
    let modules_root = root.join("modules");
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
            if path
                .components()
                .any(|component| component.as_os_str() == "github")
            {
                continue;
            }
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
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("list")
            {
                continue;
            }
            let content = match std::fs::read_to_string(&path) {
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
        Ok((artifacts, errors)) if errors == 0 => {
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
