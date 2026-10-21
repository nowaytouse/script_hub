//! Private same-executable worker dispatch. Every mutation retains its OS lock.

use super::job::{acquire_write_lock, validate_root};
use super::progress::{Stage, emit_stage};
use super::repository;
use std::path::Path;
use std::process::ExitCode;

pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let operation = args.next().unwrap_or_default();
    let arguments: Vec<String> = args.collect();
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
    let _write_lock = if operation != "validate" {
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
        "catalog" => {
            if arguments.len() != 2 || arguments[0] != "--root" {
                return fail("catalog expects only --root <repository>".to_string());
            }
            emit_stage(Stage::Catalog);
            finish(
                crate::module_catalog::refresh(&root),
                "refreshed module catalog and helper",
            )
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
