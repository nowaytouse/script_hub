use std::process::Command;

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
    if author.contains("github-actions[bot]") {
        return std::process::ExitCode::SUCCESS;
    }

    let changed = String::from_utf8_lossy(&output.stdout);
    let invalid: Vec<&str> = changed
        .lines()
        .filter(|path| {
            (path.starts_with("modules/")
                || path.starts_with("rulesets/")
                || path.starts_with("dns/"))
                && !path.starts_with("rulesets/Sources/")
        })
        .collect();
    if !invalid.is_empty() {
        eprintln!(
            "generated tree changed outside the updater: {}",
            invalid.join(", ")
        );
        return std::process::ExitCode::from(1);
    }
    std::process::ExitCode::SUCCESS
}
