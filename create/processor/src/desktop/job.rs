use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::desktop::progress::{EVENT_PREFIX, ProgressEvent, Stage};

const MAX_LOG_LINES: usize = 1_000;
const MAX_LOG_BYTES: usize = 512 * 1024;
const MAX_LINE_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobKind {
    Validate,
    RefreshCatalog,
    Update,
    QuickUpdate,
    GamingOnly,
    GitPull,
    GitCommit,
    GitPush,
}

impl JobKind {
    fn is_mutating(self) -> bool {
        !matches!(self, Self::Validate)
    }
}

#[derive(Clone, Debug, Default)]
pub struct JobOptions {
    pub root: PathBuf,
    pub singbox: Option<PathBuf>,
    pub commit_message: Option<String>,
    pub paths: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub running: bool,
    pub status: String,
    pub logs: VecDeque<String>,
    pub stage: Option<Stage>,
    pub progress: Option<ProgressEvent>,
    pub outcome: Option<JobOutcome>,
    pub elapsed: Duration,
    pub exit_code: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobOutcome {
    Success,
    Warnings,
    Failed,
}

struct State {
    running: bool,
    status: String,
    logs: VecDeque<String>,
    log_bytes: usize,
    has_warnings: bool,
    progress: Option<ProgressEvent>,
    outcome: Option<JobOutcome>,
    started_at: Instant,
    finished_at: Option<Instant>,
    exit_code: Option<i32>,
}

impl State {
    fn new(status: String) -> Self {
        Self {
            running: true,
            status,
            logs: VecDeque::new(),
            log_bytes: 0,
            has_warnings: false,
            progress: None,
            outcome: None,
            started_at: Instant::now(),
            finished_at: None,
            exit_code: None,
        }
    }

    fn push(&mut self, line: String) {
        let line = plain_log_line(&line);
        self.has_warnings |= line.contains("[WARN]");
        let bytes = line.len();
        self.logs.push_back(line);
        self.log_bytes += bytes;
        while self.logs.len() > MAX_LOG_LINES || self.log_bytes > MAX_LOG_BYTES {
            if let Some(old) = self.logs.pop_front() {
                self.log_bytes -= old.len();
            }
        }
    }

    fn elapsed(&self) -> Duration {
        self.finished_at
            .unwrap_or_else(Instant::now)
            .saturating_duration_since(self.started_at)
    }
}

fn plain_log_line(line: &str) -> String {
    let mut plain = String::with_capacity(line.len());
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' {
            if characters.next() == Some('[') {
                for code in characters.by_ref() {
                    if ('@'..='~').contains(&code) {
                        break;
                    }
                }
            }
        } else if !character.is_control() || character == '\t' {
            plain.push(character);
        }
    }
    plain
}

pub struct Job {
    state: Arc<Mutex<State>>,
    worker: Option<JoinHandle<()>>,
}

impl Job {
    pub fn clear_logs(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.logs.clear();
        state.log_bytes = 0;
    }

    pub fn start(kind: JobKind, options: JobOptions) -> Result<Self, String> {
        let root = validate_root(&options.root)?;
        let singbox = match (kind, options.singbox.clone()) {
            (JobKind::Update | JobKind::QuickUpdate | JobKind::GamingOnly, Some(path)) => {
                let path = path.canonicalize().map_err(|error| {
                    format!(
                        "cannot resolve sing-box executable {}: {error}",
                        path.display()
                    )
                })?;
                if !path.is_file() {
                    return Err(format!(
                        "sing-box executable is not a file: {}",
                        path.display()
                    ));
                }
                Some(path)
            }
            _ => None,
        };

        let executable = std::env::current_exe()
            .map_err(|error| format!("cannot locate GUI worker executable: {error}"))?;
        let mut command = Command::new(executable);
        command
            .arg("--worker")
            .arg(worker_name(kind))
            .arg("--root")
            .arg(&root)
            .current_dir(&root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if kind.is_mutating()
            && kind != JobKind::RefreshCatalog
            && let Some(path) = singbox
        {
            command.arg("--singbox").arg(path);
        }
        add_job_arguments(&mut command, kind, &options)?;
        command.env("SCRIPT_HUB_GUI_EVENTS", "1");

        let state = Arc::new(Mutex::new(State::new(match kind {
            JobKind::Validate => "校验正在运行".to_string(),
            JobKind::RefreshCatalog => "目录刷新正在运行".to_string(),
            JobKind::Update => "全量更新正在运行".to_string(),
            JobKind::QuickUpdate => "快速更新正在运行".to_string(),
            JobKind::GamingOnly => "游戏规则更新正在运行".to_string(),
            JobKind::GitPull => "正在拉取仓库更新".to_string(),
            JobKind::GitCommit => "正在提交所选更改".to_string(),
            JobKind::GitPush => "正在推送仓库更新".to_string(),
        })));
        let worker_state = Arc::clone(&state);
        let worker = thread::Builder::new()
            .name("script-hub-gui-job".to_string())
            .spawn(move || {
                let result = run_worker(command, Arc::clone(&worker_state));
                let mut state = worker_state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                finish_state(&mut state, result);
            })
            .map_err(|error| format!("cannot create job monitor: {error}"))?;
        Ok(Self {
            state,
            worker: Some(worker),
        })
    }

    pub fn is_running(&self) -> bool {
        self.reconcile_finished_monitor();
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .running
    }

    pub fn snapshot(&self) -> Snapshot {
        self.reconcile_finished_monitor();
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Snapshot {
            running: state.running,
            status: state.status.clone(),
            logs: state.logs.clone(),
            stage: state.progress.map(|progress| progress.stage),
            progress: state.progress,
            outcome: state.outcome,
            elapsed: state.elapsed(),
            exit_code: state.exit_code,
        }
    }

    fn reconcile_finished_monitor(&self) {
        if self.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.running {
                state.running = false;
                state.status = "任务监控异常退出".to_string();
                state.push("[GUI ERROR] worker monitor stopped unexpectedly".to_string());
                state.outcome = Some(JobOutcome::Failed);
                state.finished_at = Some(Instant::now());
            }
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if !self.is_running()
            && let Some(worker) = self.worker.take()
        {
            let _ = worker.join();
        }
    }
}

fn worker_name(kind: JobKind) -> &'static str {
    match kind {
        JobKind::Validate => "validate",
        JobKind::RefreshCatalog => "catalog",
        JobKind::Update | JobKind::QuickUpdate => "update",
        JobKind::GamingOnly => "update",
        JobKind::GitPull => "git-pull",
        JobKind::GitCommit => "git-commit",
        JobKind::GitPush => "git-push",
    }
}

fn add_job_arguments(
    command: &mut Command,
    kind: JobKind,
    options: &JobOptions,
) -> Result<(), String> {
    if matches!(
        kind,
        JobKind::Update | JobKind::QuickUpdate | JobKind::GamingOnly
    ) {
        command.arg("--execute");
    }
    match kind {
        JobKind::QuickUpdate => {
            command.arg("--quick");
        }
        JobKind::GamingOnly => {
            command.arg("--gaming-only");
        }
        JobKind::GitCommit => {
            let message = options
                .commit_message
                .as_deref()
                .filter(|message| !message.trim().is_empty())
                .ok_or_else(|| "commit message must not be empty".to_string())?;
            command.arg("--message").arg(message);
            for path in &options.paths {
                if path.trim().is_empty() {
                    return Err("commit path must not be empty".to_string());
                }
                command.arg("--path").arg(path);
            }
        }
        JobKind::Validate
        | JobKind::RefreshCatalog
        | JobKind::Update
        | JobKind::GitPull
        | JobKind::GitPush => {}
    }
    Ok(())
}

fn run_worker(mut command: Command, state: Arc<Mutex<State>>) -> Result<(), String> {
    (|| {
        let mut child = command
            .spawn()
            .map_err(|error| format!("cannot start GUI worker: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "worker stdout unavailable".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "worker stderr unavailable".to_string())?;

        let out_state = Arc::clone(&state);
        let out_reader = match thread::Builder::new()
            .name("script-hub-stdout".to_string())
            .spawn(move || drain(stdout, out_state, OutputStream::Stdout))
        {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("cannot monitor worker stdout: {error}"));
            }
        };
        let err_state = Arc::clone(&state);
        let err_reader = match thread::Builder::new()
            .name("script-hub-stderr".to_string())
            .spawn(move || drain(stderr, err_state, OutputStream::Stderr))
        {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_reader.join();
                return Err(format!("cannot monitor worker stderr: {error}"));
            }
        };

        let status = child
            .wait()
            .map_err(|error| format!("cannot wait for worker: {error}"));
        if let Ok(status) = &status {
            state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .exit_code = status.code();
        }
        let out_result = out_reader
            .join()
            .unwrap_or_else(|_| Err("stdout reader panicked".to_string()));
        let err_result = err_reader
            .join()
            .unwrap_or_else(|_| Err("stderr reader panicked".to_string()));
        out_result.and(err_result).and(status).and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("worker exited with {status}"))
            }
        })
    })()
}

// The worker owns this handle, so closing the GUI cannot unlock a live writer.
pub fn acquire_write_lock(root: &Path) -> Result<File, String> {
    let directory = root.join(".cache");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create GUI job lock directory: {error}"))?;
    let path = directory.join("script-hub-gui.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| format!("cannot open GUI job lock {}: {error}", path.display()))?;
    file.try_lock().map_err(|error| {
        format!(
            "another GUI write job may be active ({}): {error}",
            path.display()
        )
    })?;
    Ok(file)
}

pub fn validate_root(root: &Path) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("cannot open project directory {}: {error}", root.display()))?;
    for relative in [
        Path::new("create/processor/Cargo.toml"),
        Path::new("rulesets/Sources/adblock_whitelist.list"),
    ] {
        if !root.join(relative).is_file() {
            return Err(format!(
                "project directory is missing {}",
                relative.display()
            ));
        }
    }
    if !root.join("modules").is_dir() {
        return Err("project directory is missing modules/".to_string());
    }
    Ok(root)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum OutputStream {
    Stdout,
    Stderr,
}

fn drain(
    mut reader: impl Read,
    state: Arc<Mutex<State>>,
    stream: OutputStream,
) -> Result<(), String> {
    let mut buffer = [0; 4096];
    let mut line = Vec::with_capacity(256);
    let mut truncated = false;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("cannot read worker output: {error}"))?;
        if count == 0 {
            if !line.is_empty() || truncated {
                push_line(&state, &line, truncated, stream);
            }
            return Ok(());
        }
        for byte in &buffer[..count] {
            if *byte == b'\n' {
                push_line(&state, &line, truncated, stream);
                line.clear();
                truncated = false;
            } else if line.len() < MAX_LINE_BYTES {
                line.push(*byte);
            } else {
                truncated = true;
            }
        }
    }
}

fn push_line(state: &Arc<Mutex<State>>, bytes: &[u8], truncated: bool, stream: OutputStream) {
    let mut line = String::from_utf8_lossy(bytes)
        .trim_end_matches('\r')
        .to_string();
    if truncated {
        line.push_str(" …[行已截断]");
    }
    let mut state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(payload) = line.strip_prefix(EVENT_PREFIX) {
        if !truncated && stream == OutputStream::Stdout {
            let payload = payload.trim();
            if let Ok(event) = serde_json::from_str::<ProgressEvent>(payload)
                && event
                    .completed
                    .zip(event.total)
                    .is_none_or(|(completed, total)| completed <= total)
            {
                state.progress = Some(event);
                return;
            }
        }
        state.has_warnings = true;
        state.push(format!("[WARN] invalid GUI progress record: {line}"));
        return;
    }
    state.push(line);
}

fn finish_state(state: &mut State, result: Result<(), String>) {
    state.status = match result {
        Ok(()) if state.has_warnings => {
            state.outcome = Some(JobOutcome::Warnings);
            "已完成（有警告）".to_string()
        }
        Ok(()) => {
            state.outcome = Some(JobOutcome::Success);
            "已完成".to_string()
        }
        Err(error) => {
            state.push(format!("[GUI ERROR] {error}"));
            state.outcome = Some(JobOutcome::Failed);
            "失败".to_string()
        }
    };
    state.running = false;
    state.finished_at = Some(Instant::now());
}

#[cfg(test)]
mod tests {
    use super::{
        EVENT_PREFIX, JobKind, JobOptions, JobOutcome, MAX_LOG_BYTES, OutputStream, Stage, State,
        acquire_write_lock, add_job_arguments, drain, finish_state, run_worker, validate_root,
        worker_name,
    };
    use std::io::{self, Read};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn log_colors_are_removed_without_damaging_unicode() {
        assert_eq!(
            super::plain_log_line("\u{1b}[0;34m[INFO]\u{1b}[0m 中文\r"),
            "[INFO] 中文"
        );
    }

    #[test]
    fn log_buffer_stays_within_line_and_byte_limits() {
        let mut state = State::new(String::new());
        state.push("[WARN] warning before enough output to evict it".to_string());
        for index in 0..1_100 {
            state.push(format!("行-{index}"));
        }
        assert_eq!(state.logs.len(), 1_000);
        assert!(state.logs.front().unwrap().starts_with("行-100"));
        assert!(state.has_warnings);

        for _ in 0..600 {
            state.push("x".repeat(1_024));
        }
        assert!(state.log_bytes <= MAX_LOG_BYTES);
    }

    #[test]
    fn clearing_logs_preserves_warning_outcome_and_accepts_new_output() {
        let mut state = State::new(String::new());
        state.push("[WARN] keep this in the final outcome".to_string());
        let job = super::Job {
            state: Arc::new(Mutex::new(state)),
            worker: None,
        };
        job.clear_logs();
        let mut state = job.state.lock().unwrap();
        assert!(state.logs.is_empty());
        assert_eq!(state.log_bytes, 0);
        assert!(state.has_warnings);
        state.push("new output".to_string());
        assert_eq!(state.logs.back().unwrap(), "new output");
        assert_eq!(state.log_bytes, "new output".len());
    }

    #[test]
    fn output_reader_errors_are_returned() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("fixture read failure"))
            }
        }

        let state = Arc::new(Mutex::new(State::new(String::new())));
        assert!(
            drain(FailingReader, state, OutputStream::Stdout)
                .unwrap_err()
                .contains("fixture read failure")
        );
    }

    #[test]
    fn root_requires_project_markers_and_canonicalizes() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-gui-root-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("create/processor")).unwrap();
        std::fs::create_dir_all(root.join("rulesets/Sources")).unwrap();
        std::fs::create_dir_all(root.join("modules")).unwrap();
        std::fs::write(root.join("create/processor/Cargo.toml"), "").unwrap();
        std::fs::write(root.join("rulesets/Sources/adblock_whitelist.list"), "").unwrap();
        assert_eq!(validate_root(&root).unwrap(), root.canonicalize().unwrap());
        std::fs::remove_dir_all(root).unwrap();
        assert!(validate_root(Path::new("definitely-not-a-project-root")).is_err());
    }

    #[test]
    fn readonly_root_validation_does_not_release_writer_lock() {
        let root = std::env::temp_dir().join(format!(
            "script-hub-gui-lock-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("create/processor")).unwrap();
        std::fs::create_dir_all(root.join("rulesets/Sources")).unwrap();
        std::fs::create_dir_all(root.join("modules")).unwrap();
        std::fs::write(root.join("create/processor/Cargo.toml"), "").unwrap();
        std::fs::write(root.join("rulesets/Sources/adblock_whitelist.list"), "").unwrap();
        let writer = acquire_write_lock(&root).unwrap();
        let state = Arc::new(Mutex::new(State::new(String::new())));
        let mut readonly_worker = Command::new(std::env::current_exe().unwrap());
        readonly_worker
            .arg("--help")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        run_worker(readonly_worker, state).unwrap();
        assert!(acquire_write_lock(&root).is_err());
        drop(writer);
        assert!(acquire_write_lock(&root).is_ok());
        assert!(root.join(".cache/script-hub-gui.lock").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn operations_are_explicit() {
        assert!(!JobKind::Validate.is_mutating());
        assert!(JobKind::Update.is_mutating());
        assert!(JobKind::GamingOnly.is_mutating());
        let _options = JobOptions {
            root: Path::new(".").to_path_buf(),
            singbox: None,
            ..JobOptions::default()
        };
    }

    #[test]
    fn update_job_arguments_map_to_existing_cli_modes() {
        let mut quick = Command::new("worker");
        add_job_arguments(&mut quick, JobKind::QuickUpdate, &JobOptions::default()).unwrap();
        assert_eq!(
            quick
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["--execute".to_string(), "--quick".to_string()]
        );

        let mut gaming = Command::new("worker");
        add_job_arguments(&mut gaming, JobKind::GamingOnly, &JobOptions::default()).unwrap();
        assert_eq!(
            gaming
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["--execute".to_string(), "--gaming-only".to_string()]
        );

        assert_eq!(worker_name(JobKind::GitPull), "git-pull");
        assert_eq!(worker_name(JobKind::GitCommit), "git-commit");
        assert_eq!(worker_name(JobKind::GitPush), "git-push");

        let options = JobOptions {
            commit_message: Some("message with spaces".to_string()),
            paths: vec![
                "modules/custom source.list".to_string(),
                "rulesets/one.list".to_string(),
            ],
            ..JobOptions::default()
        };
        let mut commit = Command::new("worker");
        add_job_arguments(&mut commit, JobKind::GitCommit, &options).unwrap();
        assert_eq!(
            commit
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec![
                "--message".to_string(),
                "message with spaces".to_string(),
                "--path".to_string(),
                "modules/custom source.list".to_string(),
                "--path".to_string(),
                "rulesets/one.list".to_string(),
            ]
        );
        assert!(
            add_job_arguments(
                &mut Command::new("worker"),
                JobKind::GitCommit,
                &JobOptions::default()
            )
            .is_err()
        );
    }

    #[test]
    fn stdout_progress_records_update_stage_but_stderr_does_not() {
        let state = Arc::new(Mutex::new(State::new(String::new())));
        let record =
            format!("{EVENT_PREFIX}{{\"stage\":\"validate\",\"completed\":2,\"total\":5}}\n");
        drain(record.as_bytes(), Arc::clone(&state), OutputStream::Stdout).unwrap();
        assert_eq!(
            state.lock().unwrap().progress.unwrap().stage,
            Stage::Validate
        );

        let stderr_record = format!("{EVENT_PREFIX}{record}");
        drain(
            stderr_record.as_bytes(),
            Arc::clone(&state),
            OutputStream::Stderr,
        )
        .unwrap();
        let state = state.lock().unwrap();
        assert_eq!(state.progress.unwrap().stage, Stage::Validate);
        assert!(state.has_warnings);
        assert!(
            state
                .logs
                .back()
                .unwrap()
                .contains("invalid GUI progress record")
        );
    }

    #[test]
    fn invalid_progress_records_are_logged_and_mark_warnings() {
        let state = Arc::new(Mutex::new(State::new(String::new())));
        let record = format!("{EVENT_PREFIX}not-json\n");
        drain(record.as_bytes(), Arc::clone(&state), OutputStream::Stdout).unwrap();
        let invalid_counts =
            format!("{EVENT_PREFIX}{{\"stage\":\"validate\",\"completed\":4,\"total\":2}}\n");
        drain(
            invalid_counts.as_bytes(),
            Arc::clone(&state),
            OutputStream::Stdout,
        )
        .unwrap();
        let state = state.lock().unwrap();
        assert!(state.has_warnings);
        assert_eq!(state.logs.len(), 2);
        assert!(state.logs.front().unwrap().contains("not-json"));
        assert!(state.logs.back().unwrap().contains("\"completed\":4"));
        assert!(state.progress.is_none());
    }

    #[test]
    fn terminal_outcome_elapsed_and_exit_code_are_snapshot_ready() {
        let mut state = State::new(String::new());
        state.exit_code = Some(0);
        finish_state(&mut state, Ok(()));
        assert_eq!(state.outcome, Some(JobOutcome::Success));
        assert!(!state.running);
        assert!(state.finished_at.unwrap() >= state.started_at);
        assert!(state.elapsed() < Duration::from_secs(1));
        assert_eq!(state.exit_code, Some(0));

        let mut failed = State::new(String::new());
        failed.exit_code = Some(7);
        finish_state(&mut failed, Err("worker exited with status 7".to_string()));
        assert_eq!(failed.outcome, Some(JobOutcome::Failed));
        assert_eq!(failed.exit_code, Some(7));

        let mut warned = State::new(String::new());
        warned.push("[WARN] fixture warning".to_string());
        finish_state(&mut warned, Ok(()));
        assert_eq!(warned.outcome, Some(JobOutcome::Warnings));
    }
}
