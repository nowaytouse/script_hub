use std::process::Command;

fn generated_path(path: &str) -> bool {
    (path.starts_with("modules/") && !path.starts_with("modules/source/"))
        || (path.starts_with("rulesets/") && !path.starts_with("rulesets/Sources/"))
        || path.starts_with("dns/")
}

fn invalid_generated_paths(changed: &str, bot_commit: bool) -> Vec<&str> {
    if bot_commit
        || changed
            .lines()
            .any(|path| path.starts_with("create/processor/"))
    {
        return Vec::new();
    }
    changed
        .lines()
        .filter(|path| generated_path(path))
        .collect()
}

fn main() -> std::process::ExitCode {
    let output = match Command::new("git")
        .args(["diff", "--name-only", "HEAD~1", "HEAD"])
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

    let author = Command::new("git")
        .args(["show", "-s", "--format=%an <%ae>", "HEAD"])
        .output()
        .ok()
        .map(|value| String::from_utf8_lossy(&value.stdout).to_ascii_lowercase())
        .unwrap_or_default();
    let changed = String::from_utf8_lossy(&output.stdout);
    let invalid = invalid_generated_paths(&changed, author.contains("github-actions[bot]"));
    if !invalid.is_empty() {
        eprintln!(
            "generated tree changed outside the updater: {}",
            invalid.join(", ")
        );
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
                "modules/source/local/input.sgmodule\nrulesets/Sources/input.list\n",
                false
            )
            .is_empty()
        );
        assert!(invalid_generated_paths("modules/surge/output.sgmodule\n", true).is_empty());
        assert!(
            invalid_generated_paths(
                "create/processor/src/update.rs\nmodules/surge/output.sgmodule\n",
                false
            )
            .is_empty()
        );
        assert_eq!(
            invalid_generated_paths("modules/surge/output.sgmodule\n", false),
            ["modules/surge/output.sgmodule"]
        );
    }
}
