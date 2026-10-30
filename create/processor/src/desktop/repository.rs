use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

const MAX_STATUS_BYTES: usize = 8 * 1024 * 1024;
const MAX_COMMAND_BYTES: usize = 256 * 1024;
const MAX_CAPTURE_READ: usize = 16 * 1024;
const MAX_REMOTE_URLS: usize = 8;
const MAX_REMOTE_URL_BYTES: usize = 2048;
const DEFAULT_SSH_COMMAND: &str =
    "ssh -o BatchMode=yes -o ConnectTimeout=15 -o ServerAliveInterval=5 -o ServerAliveCountMax=2";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Change {
    pub path: String,
    pub previous_path: Option<String>,
    /// Git porcelain-v1 XY status, or `??` for an untracked path.
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepositoryStatus {
    pub branch: String,
    /// Sanitized fetch URL for `origin`, or an empty string if no origin exists.
    pub remote: String,
    /// Sanitized push destination for `origin`, which may differ from `remote`.
    pub push_remote: String,
    pub ahead: u64,
    pub behind: u64,
    pub changes: Vec<Change>,
}

#[derive(Debug)]
struct Captured {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
struct GitOutput {
    code: Option<i32>,
    stdout: Captured,
    stderr: Captured,
}

impl GitOutput {
    fn checked(self, action: &str) -> Result<Vec<u8>, String> {
        if self.stdout.truncated || self.stderr.truncated {
            return Err(format!("git {action} output exceeded the capture limit"));
        }
        if self.code != Some(0) {
            let stderr = String::from_utf8_lossy(&self.stderr.bytes);
            let detail = redact_url_credentials(stderr.trim());
            let detail = if detail.is_empty() {
                "no diagnostic was returned".to_string()
            } else {
                detail
            };
            return Err(format!(
                "git {action} failed (exit {:?}): {detail}",
                self.code
            ));
        }
        Ok(self.stdout.bytes)
    }
}

fn capture<R: Read>(mut reader: R, limit: usize) -> io::Result<Captured> {
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    let mut truncated = false;
    let mut buffer = [0_u8; MAX_CAPTURE_READ];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let retained = (limit - bytes.len()).min(count);
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained != count;
    }
    Ok(Captured { bytes, truncated })
}

fn run_git(root: &Path, args: &[OsString], limit: usize) -> Result<GitOutput, String> {
    run_git_with_ssh(root, args, limit, None)
}

fn run_git_with_ssh(
    root: &Path,
    args: &[OsString],
    limit: usize,
    default_ssh_command: Option<&str>,
) -> Result<GitOutput, String> {
    let mut command = Command::new("git");
    command
        .arg("-c")
        .arg("credential.interactive=false")
        .arg("-c")
        .arg("core.askPass=")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS")
        .env("SSH_ASKPASS_REQUIRE", "never")
        // Git documents these environment overrides for aborting stalled HTTP transfers.
        // They do not bound DNS/connect time or arbitrary custom SSH commands.
        .env("GIT_HTTP_LOW_SPEED_LIMIT", "1000")
        .env("GIT_HTTP_LOW_SPEED_TIME", "30");
    if let Some(ssh_command) = default_ssh_command {
        command.env("GIT_SSH_COMMAND", ssh_command);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start Git for {}: {error}", root.display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Git stdout pipe unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Git stderr pipe unavailable".to_string())?;

    let stdout_limit = limit;
    let stdout_reader = thread::Builder::new()
        .name("script-hub-git-stdout".to_string())
        .spawn(move || capture(stdout, stdout_limit))
        .map_err(|error| {
            let _ = child.kill();
            let _ = child.wait();
            format!("cannot monitor Git stdout: {error}")
        })?;
    let stderr_reader = match thread::Builder::new()
        .name("script-hub-git-stderr".to_string())
        .spawn(move || capture(stderr, MAX_COMMAND_BYTES))
    {
        Ok(reader) => reader,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            return Err(format!("cannot monitor Git stderr: {error}"));
        }
    };

    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for Git in {}: {error}", root.display()))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| "Git stdout monitor panicked".to_string())?
        .map_err(|error| format!("cannot read Git stdout: {error}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Git stderr monitor panicked".to_string())?
        .map_err(|error| format!("cannot read Git stderr: {error}"))?;

    Ok(GitOutput {
        code: status.code(),
        stdout,
        stderr,
    })
}

fn run_git_network(root: &Path, args: &[OsString], limit: usize) -> Result<GitOutput, String> {
    let has_custom_ssh_environment = ["GIT_SSH_COMMAND", "GIT_SSH"]
        .iter()
        .any(|name| std::env::var_os(name).is_some());
    let custom_ssh_variant = std::env::var("GIT_SSH_VARIANT")
        .ok()
        .is_some_and(|variant| !variant.eq_ignore_ascii_case("ssh"));
    if has_custom_ssh_environment || custom_ssh_variant {
        return run_git(root, args, limit);
    }

    let configured_ssh = run_git(
        root,
        &os_args(&["config", "--get", "core.sshCommand"]),
        16 * 1024,
    )?;
    match configured_ssh.code {
        Some(0) => return run_git(root, args, limit),
        Some(1) => {}
        _ => {
            configured_ssh.checked("config --get core.sshCommand")?;
        }
    }
    let configured_variant = run_git(
        root,
        &os_args(&["config", "--get", "ssh.variant"]),
        16 * 1024,
    )?;
    match configured_variant.code {
        Some(0)
            if !String::from_utf8_lossy(&configured_variant.stdout.bytes)
                .trim()
                .eq_ignore_ascii_case("ssh") =>
        {
            return run_git(root, args, limit);
        }
        Some(0) => {}
        Some(1) => {}
        _ => {
            configured_variant.checked("config --get ssh.variant")?;
        }
    }
    run_git_with_ssh(root, args, limit, Some(DEFAULT_SSH_COMMAND))
}

fn os_args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn repository_root(path: &Path) -> Result<PathBuf, String> {
    let output = run_git(path, &os_args(&["rev-parse", "--show-toplevel"]), 64 * 1024)?;
    let bytes = output.checked("rev-parse --show-toplevel")?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("Git returned a non-UTF-8 repository path: {error}"))?;
    let root = PathBuf::from(text.trim());
    root.canonicalize()
        .map_err(|error| format!("cannot resolve Git worktree {}: {error}", root.display()))
}

fn checked_text(root: &Path, args: &[&str], action: &str) -> Result<String, String> {
    let output = run_git(root, &os_args(args), 64 * 1024)?;
    let bytes = output.checked(action)?;
    String::from_utf8(bytes)
        .map_err(|error| format!("git {action} returned non-UTF-8 output: {error}"))
}

fn branch_name(root: &Path) -> Result<String, String> {
    let output = run_git(
        root,
        &os_args(&["symbolic-ref", "--quiet", "--short", "HEAD"]),
        64 * 1024,
    )?;
    if output.code == Some(1) && !output.stdout.truncated && !output.stderr.truncated {
        return Ok("(detached)".to_string());
    }
    let bytes = output.checked("symbolic-ref --short HEAD")?;
    let branch = String::from_utf8(bytes)
        .map_err(|error| format!("Git returned a non-UTF-8 branch name: {error}"))?;
    let branch = branch.trim().to_string();
    if branch.is_empty() {
        return Err("Git returned an empty branch name".to_string());
    }
    Ok(branch)
}

fn origin_url(root: &Path, push: bool) -> Result<String, String> {
    let remotes = checked_text(root, &["remote"], "remote")?;
    if !remotes.lines().any(|name| name == "origin") {
        return Ok(String::new());
    }
    let urls = if push {
        checked_text(
            root,
            &["remote", "get-url", "--push", "--all", "origin"],
            "remote get-url --push",
        )?
    } else {
        checked_text(
            root,
            &["remote", "get-url", "--all", "origin"],
            "remote get-url",
        )?
    };
    let sanitized = urls
        .lines()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .collect::<Vec<_>>();
    if sanitized.len() > MAX_REMOTE_URLS {
        return Err(format!(
            "origin has more than {MAX_REMOTE_URLS} URLs; refusing to display an oversized remote list"
        ));
    }
    sanitized
        .into_iter()
        .map(|url| {
            if url.len() > MAX_REMOTE_URL_BYTES {
                return Err(format!(
                    "origin URL exceeds the {MAX_REMOTE_URL_BYTES}-byte display limit"
                ));
            }
            Ok(sanitize_remote(url))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|urls| urls.join(", "))
}

fn sanitize_remote(remote: &str) -> String {
    if remote.contains("://") {
        return crate::redact_url(remote);
    }

    // SSH scp-style remotes use `user@host:path`; hide the user component too.
    let scp_remote = if let Some((user, host_path)) = remote.split_once('@')
        && !user.is_empty()
        && let Some((host, _)) = host_path.split_once(':')
        && !host.contains('/')
    {
        Some(host_path)
    } else if let Some((host, _)) = remote.split_once(':')
        && !host.is_empty()
        && !host.contains('/')
    {
        Some(remote)
    } else {
        None
    };
    if let Some(scp_remote) = scp_remote {
        return scp_remote
            .split(['?', '#'])
            .next()
            .unwrap_or(scp_remote)
            .to_string();
    }
    remote.to_string()
}

fn redact_url_credentials(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(scheme_end) = rest.find("://") {
        let scheme_start = rest[..scheme_end]
            .rfind(|character: char| {
                character.is_whitespace() || matches!(character, '\'' | '"' | '(' | '[' | '=')
            })
            .map_or(0, |index| index + 1);
        output.push_str(&rest[..scheme_start]);
        let url_start = scheme_start;
        let url_end = rest[url_start..]
            .find(|character: char| {
                character.is_whitespace()
                    || matches!(character, '\'' | '"' | '(' | ')' | '[' | ']' | '<' | '>')
            })
            .map_or(rest.len(), |index| url_start + index);
        output.push_str(&sanitize_remote(&rest[url_start..url_end]));
        rest = &rest[url_end..];
    }
    output.push_str(rest);
    redact_scp_remote_users(&output)
}

fn redact_scp_remote_users(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut start = 0;
    for (index, character) in text.char_indices() {
        if character.is_whitespace()
            || matches!(character, '\'' | '"' | '(' | ')' | '[' | ']' | '<' | '>')
        {
            output.push_str(&sanitize_remote(&text[start..index]));
            output.push(character);
            start = index + character.len_utf8();
        }
    }
    output.push_str(&sanitize_remote(&text[start..]));
    output
}

fn ahead_behind(root: &Path) -> Result<(u64, u64), String> {
    let branch = branch_name(root)?;
    if branch == "(detached)" {
        return Ok((0, 0));
    }
    let mut reference = OsString::from("refs/heads/");
    reference.push(&branch);
    let format = OsString::from("--format=%(upstream:short)");
    let upstream_output = run_git(
        root,
        &[OsString::from("for-each-ref"), format, reference],
        64 * 1024,
    )?;
    let upstream = String::from_utf8(upstream_output.checked("for-each-ref")?)
        .map_err(|error| format!("git for-each-ref returned non-UTF-8 output: {error}"))?;
    let upstream = upstream.trim();
    if upstream.is_empty() {
        return Ok((0, 0));
    }
    let comparison = format!("{upstream}...HEAD");
    let mut args = os_args(&["rev-list", "--left-right", "--count"]);
    args.push(OsString::from(comparison));
    let counts = run_git(root, &args, 64 * 1024)?.checked("rev-list --left-right --count")?;
    let counts = String::from_utf8(counts)
        .map_err(|error| format!("Git returned invalid ahead/behind counts: {error}"))?;
    let mut values = counts.split_whitespace();
    let behind = values
        .next()
        .ok_or_else(|| "Git omitted the behind count".to_string())?
        .parse::<u64>()
        .map_err(|error| format!("Git returned an invalid behind count: {error}"))?;
    let ahead = values
        .next()
        .ok_or_else(|| "Git omitted the ahead count".to_string())?
        .parse::<u64>()
        .map_err(|error| format!("Git returned an invalid ahead count: {error}"))?;
    if values.next().is_some() {
        return Err("Git returned extra ahead/behind data".to_string());
    }
    Ok((ahead, behind))
}

fn parse_status(bytes: &[u8]) -> Result<Vec<Change>, String> {
    let fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut changes = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let record = fields[index];
        index += 1;
        if record.is_empty() {
            continue;
        }
        if record.len() < 4 || record[2] != b' ' {
            return Err("Git returned a malformed porcelain status record".to_string());
        }
        let status = std::str::from_utf8(&record[..2])
            .map_err(|error| format!("Git returned an invalid status code: {error}"))?
            .to_string();
        let path = String::from_utf8(record[3..].to_vec()).map_err(|error| {
            format!("Git path is not UTF-8 and cannot be safely selected: {error}")
        })?;
        if path.is_empty() {
            return Err("Git returned an empty path in porcelain status".to_string());
        }
        let renamed = status.contains('R') || status.contains('C');
        let previous_path = if renamed {
            let previous = fields
                .get(index)
                .copied()
                .filter(|previous| !previous.is_empty())
                .ok_or_else(|| "Git omitted the previous path for a rename/copy".to_string())?;
            index += 1;
            Some(String::from_utf8(previous.to_vec()).map_err(|error| {
                format!("previous Git path is not UTF-8 and cannot be safely selected: {error}")
            })?)
        } else {
            None
        };
        changes.push(Change {
            path,
            previous_path,
            status,
        });
    }
    Ok(changes)
}

fn path_is_safe(path: &str) -> bool {
    if path.is_empty()
        || path.chars().any(char::is_control)
        || Path::new(path).is_absolute()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.as_bytes().get(1) == Some(&b':')
    {
        return false;
    }
    !path
        .split(['/', '\\'])
        .any(|component| component == ".." || component.eq_ignore_ascii_case(".git"))
}

fn validate_paths(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("select at least one changed path to commit".to_string());
    }
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        if !path_is_safe(path) {
            return Err(format!("refusing unsafe repository path: {path:?}"));
        }
        if !seen.insert(path) {
            return Err(format!("duplicate repository path: {path:?}"));
        }
    }
    Ok(())
}

fn ensure_no_staged_changes(changes: &[Change]) -> Result<(), String> {
    if let Some(change) = changes.iter().find(|change| {
        change
            .status
            .as_bytes()
            .first()
            .is_some_and(|code| *code != b' ' && *code != b'?')
    }) {
        return Err(format!(
            "refusing Git operation while the index already contains staged changes: {} ({})",
            change.path, change.status
        ));
    }
    Ok(())
}

fn ensure_clean(status: &RepositoryStatus, action: &str) -> Result<(), String> {
    if status.changes.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "cannot {action} with a dirty worktree ({} changed path(s)); commit or discard them first",
            status.changes.len()
        ))
    }
}

/// Read repository metadata without contacting a remote or modifying the worktree.
pub fn snapshot(root: &Path) -> Result<RepositoryStatus, String> {
    let root = repository_root(root)?;
    let status = run_git(
        &root,
        &os_args(&[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--renames",
        ]),
        MAX_STATUS_BYTES,
    )?
    .checked("status --porcelain")?;
    let changes = parse_status(&status)?;
    let branch = branch_name(&root)?;
    let remote = origin_url(&root, false)?;
    let push_remote = origin_url(&root, true)?;
    let (ahead, behind) = ahead_behind(&root)?;
    Ok(RepositoryStatus {
        branch,
        remote,
        push_remote,
        ahead,
        behind,
        changes,
    })
}

/// Fast-forward-only pull for a clean, attached worktree.
pub fn pull(root: &Path) -> Result<(), String> {
    let root = repository_root(root)?;
    let status = snapshot(&root)?;
    if status.branch == "(detached)" {
        return Err("cannot pull from a detached HEAD".to_string());
    }
    ensure_clean(&status, "pull")?;
    let output = run_git_network(&root, &os_args(&["pull", "--ff-only"]), MAX_COMMAND_BYTES)?;
    output.checked("pull --ff-only").map(|_| ())
}

/// Commit explicitly selected changes without using or disturbing any pre-existing index state.
pub fn commit(root: &Path, message: &str, paths: &[String]) -> Result<(), String> {
    let message = message.trim();
    if message.is_empty() || message.chars().any(|character| character == '\0') {
        return Err("commit message must not be empty or contain NUL".to_string());
    }
    validate_paths(paths)?;
    let root = repository_root(root)?;
    let status = snapshot(&root)?;
    ensure_no_staged_changes(&status.changes)?;

    let changes = status
        .changes
        .iter()
        .map(|change| (change.path.as_str(), change))
        .collect::<std::collections::HashMap<_, _>>();
    let mut commit_paths = std::collections::HashSet::new();
    for path in paths {
        let change = changes
            .get(path.as_str())
            .copied()
            .ok_or_else(|| format!("path is not an observed worktree change: {path:?}"))?;
        commit_paths.insert(path.clone());
        if let Some(previous) = &change.previous_path {
            if !path_is_safe(previous) {
                return Err(format!(
                    "refusing unsafe previous rename path: {previous:?}"
                ));
            }
            commit_paths.insert(previous.clone());
        }
    }

    let mut commit_paths = commit_paths.into_iter().collect::<Vec<_>>();
    commit_paths.sort();
    let mut add_args = os_args(&["add", "--"]);
    for path in &commit_paths {
        add_args.push(OsString::from(format!(":(literal){path}")));
    }
    run_git(&root, &add_args, MAX_COMMAND_BYTES)?
        .checked("add selected paths")
        .map_err(|error| {
            format!("could not stage exactly the selected paths; review the index: {error}")
        })?;

    let mut args = os_args(&["commit", "--only", "-m"]);
    args.push(OsString::from(message));
    args.push(OsString::from("--"));
    for path in &commit_paths {
        args.push(OsString::from(format!(":(literal){path}")));
    }
    let result = run_git(&root, &args, MAX_COMMAND_BYTES)
        .and_then(|output| output.checked("commit --only").map(|_| ()));
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(format!(
            "commit did not complete; selected paths remain staged and the index was not changed further; review staged paths before retrying: {error}"
        )),
    }
}

/// Push the attached branch to the same-named branch on `origin`, without force.
pub fn push(root: &Path) -> Result<(), String> {
    let root = repository_root(root)?;
    let status = snapshot(&root)?;
    if status.branch == "(detached)" {
        return Err("cannot push from a detached HEAD".to_string());
    }
    ensure_clean(&status, "push")?;
    if status.push_remote.is_empty() {
        return Err("cannot push: repository has no origin remote".to_string());
    }
    let branch = status.branch;
    let valid = run_git(
        &root,
        &[
            OsString::from("check-ref-format"),
            OsString::from("--branch"),
            OsString::from(&branch),
        ],
        64 * 1024,
    )?
    .checked("check-ref-format --branch")?;
    let _ = valid;
    let destination = format!("refs/heads/{branch}:refs/heads/{branch}");
    let output = run_git_network(
        &root,
        &[
            OsString::from("push"),
            OsString::from("--"),
            OsString::from("origin"),
            OsString::from(destination),
        ],
        MAX_COMMAND_BYTES,
    )?;
    output.checked("push origin").map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{
        commit, parse_status, pull, push, redact_url_credentials, sanitize_remote, snapshot,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(label: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "script-hub-repository-{label}-{}-{timestamp}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn git(&self, args: &[&str]) -> String {
            let output = Command::new("git")
                .arg("-C")
                .arg(self.path())
                .args(args)
                .env("GIT_TERMINAL_PROMPT", "0")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn bare_origin(label: &str) -> Fixture {
        let bare = Fixture::new(&format!("{label}-origin"));
        bare.git(&["init", "--bare", "-b", "main"]);
        bare
    }

    fn clone_from_with_autocrlf(bare: &Fixture, label: &str, autocrlf: bool) -> Fixture {
        let clone = Fixture::new(label);
        let mut command = Command::new("git");
        command.arg("clone");
        if autocrlf {
            command.args(["--config", "core.autocrlf=true"]);
        }
        let output = command
            .arg("--branch")
            .arg("main")
            .arg(bare.path())
            .arg(clone.path())
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Command::new("git")
            .arg("-C")
            .arg(clone.path())
            .args(["config", "user.name", "Script Hub Test"])
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(clone.path())
            .args(["config", "user.email", "script-hub@example.invalid"])
            .output()
            .unwrap();
        clone
    }

    fn seeded_remote(label: &str) -> (Fixture, Fixture) {
        seeded_remote_with_autocrlf(label, false)
    }

    fn seeded_remote_with_autocrlf(label: &str, autocrlf: bool) -> (Fixture, Fixture) {
        let bare = bare_origin(label);
        let seed = Fixture::new(&format!("{label}-seed"));
        seed.git(&["init", "-b", "main"]);
        seed.git(&["config", "user.name", "Script Hub Test"]);
        seed.git(&["config", "user.email", "script-hub@example.invalid"]);
        if autocrlf {
            seed.git(&["config", "core.autocrlf", "true"]);
        }
        fs::write(seed.path().join("tracked.txt"), "initial\n").unwrap();
        seed.git(&["add", "--", "tracked.txt"]);
        seed.git(&["commit", "-m", "initial"]);
        seed.git(&["remote", "add", "origin", bare.path().to_str().unwrap()]);
        seed.git(&["push", "--set-upstream", "origin", "main"]);
        let work = clone_from_with_autocrlf(&bare, &format!("{label}-clone"), autocrlf);
        (bare, work)
    }

    #[test]
    fn porcelain_zero_parser_keeps_renamed_paths_and_untracked_entries() {
        let bytes = b"R  new.txt\0old.txt\0?? untracked.txt\0 M changed.txt\0";
        let changes = parse_status(bytes).unwrap();
        assert_eq!(changes[0].path, "new.txt");
        assert_eq!(changes[0].previous_path.as_deref(), Some("old.txt"));
        assert_eq!(changes[1].status, "??");
        assert_eq!(changes[2].path, "changed.txt");
    }

    #[test]
    fn remote_credentials_are_not_exposed() {
        assert_eq!(
            sanitize_remote("https://user:secret@example.test/owner/repo.git"),
            "https://example.test/owner/repo.git"
        );
        assert_eq!(
            sanitize_remote(
                "https://user:secret@example.test/owner/repo.git?access_token=secret#private"
            ),
            "https://example.test/owner/repo.git"
        );
        assert_eq!(
            sanitize_remote("git@example.test:owner/repo.git"),
            "example.test:owner/repo.git"
        );
        assert_eq!(
            redact_url_credentials(
                "failed https://user:secret@example.test/repo?token=private and git@host:owner/repo"
            ),
            "failed https://example.test/repo and host:owner/repo"
        );
    }

    #[test]
    fn snapshot_reports_branch_remote_and_rename_without_network() {
        let (bare, repo) = seeded_remote("snapshot");
        repo.git(&["mv", "tracked.txt", "renamed.txt"]);
        let status = snapshot(repo.path()).unwrap();
        assert_eq!(status.branch, "main");
        assert_eq!(
            Path::new(&status.remote).canonicalize().unwrap(),
            bare.path().canonicalize().unwrap()
        );
        assert_eq!(status.push_remote, status.remote);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert_eq!(status.changes[0].path, "renamed.txt");
        assert_eq!(
            status.changes[0].previous_path.as_deref(),
            Some("tracked.txt")
        );
        drop(bare);
    }

    #[test]
    fn commit_uses_only_selected_paths_and_refuses_preexisting_index() {
        let (_bare, repo) = seeded_remote("commit");
        fs::write(repo.path().join("tracked.txt"), "updated\n").unwrap();
        fs::write(repo.path().join("new.txt"), "untracked\n").unwrap();
        fs::write(repo.path().join("unselected.txt"), "leave untracked\n").unwrap();
        let status = snapshot(repo.path()).unwrap();
        let tracked = status
            .changes
            .iter()
            .find(|change| change.path == "tracked.txt")
            .unwrap();
        assert_eq!(tracked.status, " M");
        assert!(commit(repo.path(), "test commit", &["tracked.txt".to_string()]).is_ok());
        assert_eq!(repo.git(&["show", "HEAD:tracked.txt"]), "updated");
        assert!(commit(repo.path(), "selected new file", &["new.txt".to_string()]).is_ok());
        assert_eq!(repo.git(&["show", "HEAD:new.txt"]), "untracked");
        let unselected = Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["cat-file", "-e", "HEAD:unselected.txt"])
            .output()
            .unwrap();
        assert!(!unselected.status.success());
        let remaining = repo.git(&["status", "--porcelain"]);
        assert_eq!(remaining.lines().count(), 1);
        assert!(remaining.contains("unselected.txt"));

        fs::write(repo.path().join("tracked.txt"), "staged\n").unwrap();
        repo.git(&["add", "--", "tracked.txt"]);
        let staged_before = repo.git(&["diff", "--cached", "--name-only"]);
        assert!(commit(repo.path(), "must refuse", &["tracked.txt".to_string()]).is_err());
        assert_eq!(
            repo.git(&["diff", "--cached", "--name-only"]),
            staged_before
        );
    }

    #[test]
    fn snapshot_reports_push_url_separately_and_redacts_query_secrets() {
        let (bare, repo) = seeded_remote("push-url");
        repo.git(&[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://user:secret@example.test/owner/repo.git?access_token=private",
        ]);
        let status = snapshot(repo.path()).unwrap();
        assert_eq!(
            Path::new(&status.remote).canonicalize().unwrap(),
            bare.path().canonicalize().unwrap()
        );
        assert_eq!(status.push_remote, "https://example.test/owner/repo.git");
        assert!(!status.push_remote.contains("secret"));
        assert!(!status.push_remote.contains("private"));
    }

    #[test]
    fn failed_commit_does_not_reset_or_discard_index_changes() {
        let (_bare, repo) = seeded_remote("failed-commit");
        fs::write(repo.path().join("selected.txt"), "selected\n").unwrap();
        fs::write(repo.path().join("unselected.txt"), "unselected\n").unwrap();
        repo.git(&["config", "user.name", ""]);
        repo.git(&["config", "user.email", ""]);

        let error = commit(
            repo.path(),
            "will fail without an identity",
            &["selected.txt".to_string()],
        )
        .unwrap_err();
        assert!(error.contains("selected paths remain staged"));
        assert!(error.contains("index was not changed further"));
        assert_eq!(
            repo.git(&["diff", "--cached", "--name-only"]),
            "selected.txt"
        );
        let status = repo.git(&["status", "--porcelain"]);
        assert!(status.contains("A  selected.txt"));
        assert!(status.contains("?? unselected.txt"));
    }

    #[test]
    fn push_uses_configured_push_url_and_same_named_branch() {
        let (fetch_remote, repo) = seeded_remote("push-target");
        let push_remote = bare_origin("push-target-destination");
        repo.git(&[
            "remote",
            "set-url",
            "--push",
            "origin",
            push_remote.path().to_str().unwrap(),
        ]);
        fs::write(repo.path().join("tracked.txt"), "push destination\n").unwrap();
        commit(repo.path(), "commit for push", &["tracked.txt".to_string()]).unwrap();
        let status = snapshot(repo.path()).unwrap();
        assert_eq!(status.remote, fetch_remote.path().display().to_string());
        assert_eq!(status.push_remote, push_remote.path().display().to_string());
        push(repo.path()).unwrap();
        let pushed = Command::new("git")
            .arg("--git-dir")
            .arg(push_remote.path())
            .args(["show", "refs/heads/main:tracked.txt"])
            .output()
            .unwrap();
        assert!(pushed.status.success());
        assert_eq!(
            String::from_utf8_lossy(&pushed.stdout).trim(),
            "push destination"
        );
    }

    #[test]
    fn pull_requires_a_clean_worktree_and_push_targets_origin_branch() {
        let (bare, repo) = seeded_remote_with_autocrlf("pull-push", true);
        push(repo.path()).unwrap_or_else(|error| panic!("push fixture setup failed: {error}"));
        let tracked_path = repo.path().join("tracked.txt");
        let original_bytes = fs::read(&tracked_path).unwrap();
        assert_eq!(
            original_bytes, b"initial\r\n",
            "fixture must reproduce a CRLF checkout"
        );
        fs::write(&tracked_path, "dirty\n").unwrap();
        let dirty_error = pull(repo.path()).expect_err("pull must reject a dirty worktree");
        assert!(
            dirty_error.contains("dirty worktree"),
            "expected dirty-worktree rejection, got: {dirty_error}"
        );
        fs::write(&tracked_path, original_bytes).unwrap();
        let status = repo.git(&["status", "--porcelain=v1", "--untracked-files=all"]);
        let index_contents = repo.git(&["show", "HEAD:tracked.txt"]);
        let diff = repo.git(&["diff", "--", "tracked.txt"]);
        let attributes = repo.git(&["check-attr", "--all", "--", "tracked.txt"]);
        let snapshot_status = snapshot(repo.path()).unwrap();
        assert!(
            status.is_empty() && snapshot_status.changes.is_empty(),
            "fixture restore must be clean: raw_status={status:?}, snapshot_changes={:?}, index={index_contents:?}, diff={diff:?}, attributes={attributes:?}",
            snapshot_status.changes
        );
        pull(repo.path())
            .unwrap_or_else(|error| panic!("pull after fixture restore failed: {error}"));
        let origin_refs = Command::new("git")
            .arg("--git-dir")
            .arg(bare.path())
            .args(["show-ref", "--heads", "main"])
            .output()
            .unwrap();
        assert!(origin_refs.status.success());
    }

    #[test]
    fn unsafe_and_empty_commit_path_selections_are_rejected() {
        assert!(super::validate_paths(&[]).is_err());
        for path in [
            "../outside",
            ".git/config",
            "nested/.GIT/config",
            "/etc/passwd",
            "C:\\secret",
            "line\nbreak",
        ] {
            assert!(
                super::validate_paths(&[path.to_string()]).is_err(),
                "{path:?}"
            );
        }
    }
}
