use crate as rust_processor;
use crate::desktop::progress::{Stage, emit_stage};
use std::path::Path;
use std::process::ExitCode;

fn validate_file(
    path: &Path,
) -> Result<Vec<rust_processor::promax::validation::ValidationError>, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let content = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut validation = match extension {
        "plugin" => rust_processor::promax::validation::validate_loon_plugin(&content),
        "sgmodule" => rust_processor::promax::validation::validate_surge_module(&content),
        "module" => rust_processor::promax::validation::validate_shadowrocket_module(&content),
        "list" => rust_processor::promax::validation::validate_external_ruleset(&content),
        _ => return Err(format!("unsupported validation file type: .{extension}")),
    };
    if extension == "list" {
        let actual = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("//"))
            .collect::<std::collections::HashSet<_>>()
            .len();
        let declared: Vec<_> = content
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix("# Rules:")
                    .or_else(|| line.trim().strip_prefix("# Total Rules:"))
            })
            .collect();
        if actual == 0
            || declared.len() != 1
            || declared[0].trim().parse::<usize>().ok() != Some(actual)
        {
            validation.push(rust_processor::promax::validation::ValidationError {
                line: 1,
                code: "ruleset-count",
                message: format!(
                    "expected a non-empty ruleset with its actual unique count ({actual})"
                ),
            });
        }
    }
    let adblock_ruleset = extension == "list"
        && path
            .ancestors()
            .any(|ancestor| ancestor.ends_with(Path::new("rulesets/AdBlock")));
    validation.extend(
        rust_processor::promax::validation::validate_apple_compatibility(&content, adblock_ruleset),
    );
    Ok(validation)
}

fn report_validation(
    path: &Path,
    validation: &[rust_processor::promax::validation::ValidationError],
    detail_limit: usize,
) -> usize {
    if !validation.is_empty() {
        eprintln!(
            "[ERROR] {}: {} validation error(s)",
            path.display(),
            validation.len()
        );
        for item in validation.iter().take(detail_limit) {
            eprintln!("        line {} {}", item.line, item.code);
        }
    }
    validation.len()
}

fn report_walkdir_error(error: walkdir::Error, errors: &mut usize) {
    eprintln!("[ERROR] directory traversal failed: {error}");
    *errors += 1;
}

fn validate_tree(root: &Path) -> Result<(usize, usize), String> {
    emit_stage(Stage::Validate);
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
        for entry in walkdir::WalkDir::new(directory) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report_walkdir_error(error, &mut errors);
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if matches!(extension, "sgmodule" | "module" | "plugin") {
                let validation = match validate_file(path) {
                    Ok(validation) => validation,
                    Err(error) => {
                        eprintln!("[ERROR] {}: {error}", path.display());
                        errors += 1;
                        continue;
                    }
                };
                artifacts += 1;
                errors += report_validation(path, &validation, 3);
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
        for entry in walkdir::WalkDir::new(&directory) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report_walkdir_error(error, &mut errors);
                    continue;
                }
            };
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("list")
            {
                continue;
            }
            let validation = match validate_file(path) {
                Ok(validation) => validation,
                Err(error) => {
                    eprintln!("[ERROR] {}: {error}", path.display());
                    errors += 1;
                    continue;
                }
            };
            artifacts += 1;
            errors += report_validation(path, &validation, 3);
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
    let mut boundary_errors = rust_processor::general_update::validate_category_boundaries(root)?;
    boundary_errors.extend(rust_processor::general_update::validate_published_layout(
        root,
    )?);
    for message in &boundary_errors {
        eprintln!("[ERROR] ruleset category contamination: {message}");
    }
    errors += boundary_errors.len();
    Ok((artifacts, errors))
}

pub fn run_cli(mut args: impl Iterator<Item = String>) -> ExitCode {
    match args.next().as_deref() {
        Some("--file") => match args.next() {
            Some(file) => {
                let path = Path::new(&file);
                emit_stage(Stage::Validate);
                match validate_file(path) {
                    Ok(validation) if report_validation(path, &validation, 50) == 0 => {
                        println!("[SUCCESS] validated {}", path.display());
                        ExitCode::SUCCESS
                    }
                    Ok(_) => ExitCode::from(1),
                    Err(error) => {
                        eprintln!("[ERROR] {}: {error}", path.display());
                        ExitCode::from(1)
                    }
                }
            }
            None => {
                eprintln!("Usage: validate --root <repository> | --file <module-or-ruleset>");
                ExitCode::from(2)
            }
        },
        Some("--root") => match args.next() {
            Some(root) => match validate_tree(Path::new(&root)) {
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
            },
            None => {
                eprintln!("Usage: validate --root <repository> | --file <module-or-ruleset>");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("Usage: validate --root <repository> | --file <module-or-ruleset>");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{report_walkdir_error, validate_file};

    #[test]
    fn validates_one_module_file_without_scanning_the_tree() {
        let path = std::env::temp_dir().join(format!(
            "script-hub-validate-one-{}.sgmodule",
            std::process::id()
        ));
        std::fs::write(&path, "#!name=fixture\n[Rule]\nNOT_A_SURGE_RULE\n").unwrap();

        let validation = validate_file(&path);
        std::fs::remove_file(path).unwrap();
        let validation = validation.unwrap();
        assert_eq!(validation.len(), 1);
        assert_eq!(validation[0].line, 3);
        assert_eq!(validation[0].code, "invalid-rule");
    }

    #[test]
    fn validates_shadowrocket_module_without_surge_only_ip_options() {
        let path = std::env::temp_dir().join(format!(
            "script-hub-validate-shadowrocket-{}.module",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "#!name=PROMAX fixture\n[Rule]\nIP-CIDR,192.0.2.1/32,REJECT-DROP\n",
        )
        .unwrap();

        let validation = validate_file(&path);
        std::fs::remove_file(path).unwrap();

        assert!(validation.unwrap().is_empty());
    }

    #[test]
    fn validates_one_ruleset_file() {
        let path = std::env::temp_dir().join(format!(
            "script-hub-validate-one-{}.list",
            std::process::id()
        ));
        std::fs::write(&path, "# Rules: 1\nDOMAIN-SUFFIX,ads.example\n").unwrap();

        let validation = validate_file(&path);
        std::fs::remove_file(path).unwrap();
        let validation = validation.unwrap();
        assert!(validation.is_empty());
    }

    #[test]
    fn rejects_empty_or_falsely_counted_rulesets() {
        let path = std::env::temp_dir().join(format!(
            "script-hub-validate-count-{}.list",
            std::process::id()
        ));
        for content in ["# Rules: 0\n", "# Rules: 1000\nDOMAIN,example.test\n"] {
            std::fs::write(&path, content).unwrap();
            assert!(
                validate_file(&path)
                    .unwrap()
                    .iter()
                    .any(|error| error.code == "ruleset-count")
            );
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn counts_directory_traversal_errors() {
        let missing = std::env::temp_dir().join(format!(
            "script-hub-missing-directory-{}",
            std::process::id()
        ));
        let error = walkdir::WalkDir::new(missing)
            .into_iter()
            .next()
            .unwrap()
            .unwrap_err();
        let mut errors = 0;

        report_walkdir_error(error, &mut errors);

        assert_eq!(errors, 1);
    }
}
