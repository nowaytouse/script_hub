use std::process::Command;

fn generated_path(path: &str) -> bool {
    (path.starts_with("modules/") && !path.starts_with("modules/source/"))
        || (path.starts_with("rulesets/") && !path.starts_with("rulesets/Sources/"))
        || path.starts_with("dns/")
}

fn invalid_generated_paths(changed: &str, bot_commit: bool) -> Vec<&str> {
    if bot_commit
        || changed
            .split_terminator('\0')
            .any(|path| path.starts_with("crates/processor/"))
    {
        return Vec::new();
    }
    changed
        .split_terminator('\0')
        .filter(|path| generated_path(path))
        .collect()
}

fn main() -> std::process::ExitCode {
    if rust_processor::app::print_version("guard") {
        return std::process::ExitCode::SUCCESS;
    }
    let base = std::env::var("DIFF_BASE").unwrap_or_else(|_| "HEAD~1".to_string());
    let head = std::env::var("DIFF_HEAD").unwrap_or_else(|_| "HEAD".to_string());
    let output = match Command::new("git")
        .args([
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            &base,
            &head,
            "--",
        ])
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            eprintln!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return std::process::ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("unable to run git: {error}");
            return std::process::ExitCode::from(1);
        }
    };

    let changed = String::from_utf8_lossy(&output.stdout);
    let trusted_updater =
        std::env::var("TRUSTED_UPDATER").is_ok_and(|value| value.eq_ignore_ascii_case("true"));
    let invalid = invalid_generated_paths(&changed, trusted_updater);
    if !invalid.is_empty() {
        eprintln!("generated tree changed outside the updater: {invalid:?}");
        return std::process::ExitCode::from(1);
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::invalid_generated_paths;

    #[test]
    fn permits_sources_bots_and_generator_migrations() {
        assert!(
            invalid_generated_paths(
                "modules/source/local/input.sgmodule\0rulesets/Sources/input.list\0",
                false
            )
            .is_empty()
        );
        assert!(invalid_generated_paths("modules/surge/output.sgmodule\0", true).is_empty());
        assert!(
            invalid_generated_paths(
                "crates/processor/src/update.rs\0modules/surge/output.sgmodule\0",
                false
            )
            .is_empty()
        );
        assert_eq!(
            invalid_generated_paths("modules/surge/output.sgmodule\0", false),
            ["modules/surge/output.sgmodule"]
        );
    }

    #[test]
    fn rejects_unicode_generated_paths_even_with_newlines_in_other_names() {
        assert_eq!(
            invalid_generated_paths(
                "notes/fixture\ncrates/processor/update.rs\0modules/surge/head_expanse/🌟 AdBlock Helper .sgmodule\0",
                false,
            ),
            ["modules/surge/head_expanse/🌟 AdBlock Helper .sgmodule"]
        );
    }
}
