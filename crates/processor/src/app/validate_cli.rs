use crate as rust_processor;
use crate::runtime::progress::{Stage, emit_stage};
use std::path::Path;
use std::process::ExitCode;

fn validate_file(
    path: &Path,
) -> Result<Vec<rust_processor::promax::validation::ValidationError>, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let content = read_artifact(path)?;
    let mut validation = match extension {
        "sgmodule" => rust_processor::promax::validation::validate_surge_module(&content),
        "module" => rust_processor::promax::validation::validate_shadowrocket_module(&content),
        "list" => rust_processor::promax::validation::validate_external_ruleset(&content),
        _ => return Err(format!("unsupported validation file type: .{extension}")),
    };
    if extension == "list" {
        let actual = content
            .lines()
            .filter_map(rust_processor::promax::validation::external_rule_line)
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
        let adblock_ruleset = path
            .ancestors()
            .any(|ancestor| ancestor.ends_with(Path::new("rulesets/AdBlock")));
        validation.extend(
            rust_processor::promax::validation::validate_apple_compatibility(
                &content,
                adblock_ruleset,
            ),
        );
    }
    Ok(validation)
}

fn read_artifact(path: &Path) -> Result<String, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("expected a regular artifact file, not a symbolic link or directory".into());
    }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}

fn require_publication_directory(root: &Path, directory: &Path) -> Result<(), String> {
    for path in directory.ancestors().take_while(|path| *path != root) {
        let metadata = std::fs::symlink_metadata(path).map_err(|error| {
            format!(
                "cannot inspect publication directory {}: {error}",
                path.display()
            )
        })?;
        if !metadata.is_dir() {
            return Err(format!(
                "expected a real publication directory: {}",
                path.display()
            ));
        }
    }
    Ok(())
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

fn validate_retired_outputs(root: &Path) -> Result<(), String> {
    for retired in ["modules/loon", "modules/helper"] {
        let path = root.join(retired);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Err(format!("retired output must not be published: {retired}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("cannot inspect retired output {retired}: {error}"));
            }
        }
    }
    Ok(())
}

fn validate_tree(root: &Path) -> Result<(usize, usize), String> {
    emit_stage(Stage::Validate);
    validate_retired_outputs(root)?;
    rust_processor::firewall_sync::validate_published_helpers(root)?;
    let mut artifacts = 0;
    let mut errors = 0;
    let modules_root = root.join("modules");
    let module_dirs = [
        modules_root.join("surge"),
        modules_root.join("shadowrocket"),
    ];
    let ruleset_dirs = [
        root.join("rulesets/AdBlock"),
        root.join("rulesets/RULE-SET"),
        root.join("rulesets/Sources/dns/mapping"),
    ];
    for directory in module_dirs.iter().chain(&ruleset_dirs) {
        require_publication_directory(root, directory)?;
    }
    let port_source = root.join("rulesets/Sources/conf/SurgeConf_DirectPorts.list");
    require_publication_directory(root, &root.join("rulesets/Sources/conf"))?;
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
    for directory in module_dirs {
        for entry in walkdir::WalkDir::new(directory) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report_walkdir_error(error, &mut errors);
                    continue;
                }
            };
            let path = entry.path();
            if entry.file_type().is_symlink() {
                return Err(format!(
                    "unexpected symbolic link in published modules: {}",
                    path.display()
                ));
            }
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if matches!(extension, "sgmodule" | "module") {
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

    for directory in ruleset_dirs {
        for entry in walkdir::WalkDir::new(&directory) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report_walkdir_error(error, &mut errors);
                    continue;
                }
            };
            let path = entry.path();
            if entry.file_type().is_symlink() {
                return Err(format!(
                    "unexpected symbolic link in published rulesets: {}",
                    path.display()
                ));
            }
            if path.extension().and_then(|value| value.to_str()) != Some("list") {
                continue;
            }
            if directory.ends_with("rulesets/Sources/dns/mapping")
                && rust_processor::srs_generator::is_input_manifest(path)
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

    let content = read_artifact(&port_source)
        .map_err(|error| format!("{}: {error}", port_source.display()))?;
    let validation =
        rust_processor::promax::validation::validate_apple_compatibility(&content, false);
    artifacts += 1;
    errors += report_validation(&port_source, &validation, 3);

    artifacts += 1;
    let mut boundary_errors = rust_processor::general_update::validate_category_boundaries(root)?;
    boundary_errors.extend(rust_processor::general_update::validate_published_layout(
        root,
    )?);
    for message in &boundary_errors {
        eprintln!("[ERROR] ruleset category contamination: {message}");
    }
    errors += boundary_errors.len();
    artifacts += rust_processor::mihomo_generator::validate_generated(root)?;
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
    use super::{
        report_walkdir_error, require_publication_directory, validate_file,
        validate_retired_outputs,
    };

    #[test]
    fn required_publication_directory_cannot_be_missing_or_a_file() {
        let root =
            std::env::temp_dir().join(format!("script-hub-publication-dir-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("shadowrocket");
        assert!(require_publication_directory(&root, &path).is_err());
        std::fs::write(&path, "not a directory").unwrap();
        assert!(require_publication_directory(&root, &path).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(require_publication_directory(&root, &path).is_ok());
        std::fs::remove_dir(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn validation_does_not_follow_artifact_or_publication_symlinks() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-publication-link-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let original = root.join("original.list");
        std::fs::write(&original, "# Rules: 1\nDOMAIN,example.test\n").unwrap();
        let link = root.join("fixture.list");
        std::os::unix::fs::symlink(&original, &link).unwrap();
        assert!(
            validate_file(&link)
                .unwrap_err()
                .contains("regular artifact")
        );
        let directory_link = root.join("modules");
        std::os::unix::fs::symlink(&root, &directory_link).unwrap();
        assert!(require_publication_directory(&root, &directory_link).is_err());
        for file in [link, directory_link, original] {
            std::fs::remove_file(file).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn rejects_reintroduced_retired_outputs_without_removing_local_data() {
        let root =
            std::env::temp_dir().join(format!("script-hub-retired-output-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(validate_retired_outputs(&root).is_ok());
        for retired in ["modules/loon", "modules/helper"] {
            let directory = root.join(retired);
            std::fs::create_dir_all(&directory).unwrap();
            let sentinel = directory.join("user-data");
            std::fs::write(&sentinel, "keep").unwrap();
            assert!(
                validate_retired_outputs(&root)
                    .unwrap_err()
                    .contains(retired)
            );
            assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "keep");
            std::fs::remove_dir_all(directory).unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn retired_output_guard_rejects_dangling_symlinks() {
        let root =
            std::env::temp_dir().join(format!("script-hub-retired-symlink-{}", std::process::id()));
        std::fs::create_dir_all(root.join("modules")).unwrap();
        std::os::unix::fs::symlink("missing-target", root.join("modules/loon")).unwrap();
        assert!(validate_retired_outputs(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

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
    fn validates_shadowrocket_module_with_supported_no_resolve() {
        let path = std::env::temp_dir().join(format!(
            "script-hub-validate-shadowrocket-{}.module",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "#!name=PROMAX fixture\n[Rule]\nIP-CIDR,192.0.2.1/32,REJECT-DROP,no-resolve\n",
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
        std::fs::write(
            &path,
            "# Rules: 1\n  ; ignored note\nDOMAIN-SUFFIX,ads.example\n",
        )
        .unwrap();

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
        for content in [
            "# Rules: 0\n",
            "# Rules: 1\n  ; only a comment\n",
            "# Rules: 1000\nDOMAIN,example.test\n",
        ] {
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
