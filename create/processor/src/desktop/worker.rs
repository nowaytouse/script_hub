//! Private same-executable worker dispatch. Every mutation retains its OS lock.

use super::job::{acquire_write_lock, validate_root};
use super::progress::{Stage, emit_stage};
use super::repository;
use std::path::Path;
use std::process::ExitCode;

fn adblock_arguments(arguments: &[String]) -> Result<Vec<String>, String> {
    let mut root = false;
    let mut singbox = false;
    let mut execute = false;
    let mut validated = Vec::with_capacity(arguments.len() + 1);
    let mut args = arguments.iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" | "--singbox" => {
                let is_root = argument == "--root";
                let seen = if is_root { &mut root } else { &mut singbox };
                if *seen {
                    return Err(format!("duplicate worker option {argument}"));
                }
                *seen = true;
                let value = args
                    .next()
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| format!("missing value for worker option {argument}"))?;
                validated.extend([argument.clone(), value.clone()]);
            }
            "--execute" if !execute => {
                execute = true;
                validated.push(argument.clone());
            }
            _ => return Err(format!("unsupported adblock worker option {argument}")),
        }
    }
    if !root || !execute {
        return Err("adblock worker expects --root <repository> and --execute".to_string());
    }
    validated.push("--adblock-only".to_string());
    Ok(validated)
}

fn srs_arguments(arguments: &[String]) -> Result<(String, String), String> {
    let mut root = None;
    let mut singbox = None;
    let mut args = arguments.iter();
    while let Some(argument) = args.next() {
        let slot = match argument.as_str() {
            "--root" => &mut root,
            "--singbox" => &mut singbox,
            _ => return Err(format!("unsupported SRS worker option {argument}")),
        };
        if slot.is_some() {
            return Err(format!("duplicate worker option {argument}"));
        }
        *slot = Some(
            args.next()
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| format!("missing value for worker option {argument}"))?
                .clone(),
        );
    }
    Ok((
        root.ok_or_else(|| "SRS worker requires --root <repository>".to_string())?,
        singbox.ok_or_else(|| "SRS worker requires --singbox <executable>".to_string())?,
    ))
}

pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let operation = args.next().unwrap_or_default();
    let arguments: Vec<String> = args.collect();
    let adblock_arguments = if operation == "adblock" {
        match adblock_arguments(&arguments) {
            Ok(arguments) => Some(arguments),
            Err(error) => return fail(error),
        }
    } else {
        None
    };
    let srs_singbox = if operation == "srs" {
        match srs_arguments(&arguments) {
            Ok((_, singbox)) => Some(singbox),
            Err(error) => return fail(error),
        }
    } else {
        None
    };
    let root = arguments
        .windows(2)
        .find(|pair| pair[0] == "--root")
        .ok_or_else(|| "missing --root <repository>".to_string())
        .and_then(|pair| validate_root(Path::new(&pair[1])));
    let root = match root {
        Ok(root) => root,
        Err(error) => {
            eprintln!("[ERROR] {error}");
            return ExitCode::from(2);
        }
    };
    let srs_singbox = match srs_singbox {
        Some(path) => match Path::new(&path).canonicalize() {
            Ok(path) if path.is_file() => Some(path),
            Ok(path) => {
                return fail(format!(
                    "sing-box executable is not a file: {}",
                    path.display()
                ));
            }
            Err(error) => {
                return fail(format!(
                    "cannot resolve sing-box executable {path}: {error}"
                ));
            }
        },
        None => None,
    };
    let _write_lock = if !matches!(operation.as_str(), "validate" | "catalog") {
        match acquire_write_lock(&root) {
            Ok(lock) => Some(lock),
            Err(error) => {
                eprintln!("[ERROR] {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    emit_stage(Stage::Prepare);
    match operation.as_str() {
        "validate" => crate::validate_cli::run_cli(arguments.into_iter()),
        "update" => crate::update_cli::run_cli(arguments.into_iter()),
        "adblock" => crate::update_cli::run_cli(
            adblock_arguments
                .expect("adblock arguments validated before dispatch")
                .into_iter(),
        ),
        "srs" => {
            emit_stage(Stage::Srs);
            let singbox = srs_singbox.expect("SRS executable validated before dispatch");
            if !crate::srs_generator::run_srs_generator(
                &root.to_string_lossy(),
                &singbox.to_string_lossy(),
            ) {
                return fail("SRS generation failed".to_string());
            }
            emit_stage(Stage::Validate);
            let validation = crate::validate_cli::run_cli(
                ["--root".to_string(), root.to_string_lossy().into_owned()].into_iter(),
            );
            if validation != ExitCode::SUCCESS {
                return fail(
                    "SRS generation completed but repository validation failed".to_string(),
                );
            }
            finish(Ok(()), "SRS generation and validation completed")
        }
        "catalog" => {
            if arguments.len() != 2 || arguments[0] != "--root" {
                return fail("catalog expects only --root <repository>".to_string());
            }
            emit_stage(Stage::Catalog);
            let result = [false, true].into_iter().try_for_each(|platform| {
                super::catalog::Catalog::load(&root, platform).map(|_| ())
            });
            finish(result, "validated live Surge and Shadowrocket catalogs")
        }
        "git-pull" | "git-push" | "git-commit" => {
            emit_stage(Stage::Repository);
            let mut message = None;
            let mut paths = Vec::new();
            let mut index = 0;
            while index < arguments.len() {
                let Some(value) = arguments.get(index + 1) else {
                    return fail("missing worker option value".to_string());
                };
                match arguments[index].as_str() {
                    "--root" => {}
                    "--message" if operation == "git-commit" => message = Some(value.clone()),
                    "--path" if operation == "git-commit" => paths.push(value.clone()),
                    _ => return fail("unsupported repository worker option".to_string()),
                }
                index += 2;
            }
            let result = match operation.as_str() {
                "git-pull" => repository::pull(&root),
                "git-push" => repository::push(&root),
                _ => repository::commit(&root, message.as_deref().unwrap_or_default(), &paths),
            };
            finish(result, "repository operation completed")
        }
        _ => fail("unknown worker operation".to_string()),
    }
}

fn finish(result: Result<(), String>, message: &str) -> ExitCode {
    match result {
        Ok(()) => {
            emit_stage(Stage::Finalize);
            println!("[SUCCESS] {message}");
            ExitCode::SUCCESS
        }
        Err(error) => fail(error),
    }
}

fn fail(error: String) -> ExitCode {
    eprintln!("[ERROR] {error}");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::{adblock_arguments, srs_arguments};

    #[test]
    fn adblock_worker_adds_mode_only_after_validating_its_arguments() {
        let args = [
            "--root".to_string(),
            "/repo".to_string(),
            "--execute".to_string(),
            "--singbox".to_string(),
            "/bin/sing-box".to_string(),
        ];
        assert_eq!(
            adblock_arguments(&args).unwrap(),
            [
                "--root",
                "/repo",
                "--execute",
                "--singbox",
                "/bin/sing-box",
                "--adblock-only"
            ]
            .map(str::to_string)
        );
        assert!(
            adblock_arguments(&["--root", "/repo", "--execute", "--quick"].map(str::to_string))
                .is_err()
        );
    }

    #[test]
    fn srs_worker_requires_exact_root_and_compiler_arguments() {
        assert_eq!(
            srs_arguments(&["--root", "/repo", "--singbox", "/bin/sing-box"].map(str::to_string))
                .unwrap(),
            ("/repo".to_string(), "/bin/sing-box".to_string())
        );
        assert!(srs_arguments(&["--root", "/repo"].map(str::to_string)).is_err());
        assert!(
            srs_arguments(
                &["--root", "/repo", "--singbox", "/bin/sing-box", "--execute"].map(str::to_string)
            )
            .is_err()
        );
    }
}
