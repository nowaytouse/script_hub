use std::path::Path;
use std::process::ExitCode;

fn validate_tree(root: &Path) -> Result<(usize, usize), String> {
    let mut modules = 0;
    let mut errors = 0;
    for directory in [root.join("modules"), root.join("rulesets")] {
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
                let canonical = path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .contains("Universal Ad-Blocking Rules Dependency Component PROMAX")
                });
                if !canonical {
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
                modules += 1;
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
    Ok((modules, errors))
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
        Ok((modules, errors)) if errors == 0 => {
            println!("[SUCCESS] validated {modules} module artifacts");
            ExitCode::SUCCESS
        }
        Ok((modules, errors)) => {
            eprintln!("[ERROR] {modules} module artifacts contain {errors} validation error(s)");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("[ERROR] {error}");
            ExitCode::from(1)
        }
    }
}
