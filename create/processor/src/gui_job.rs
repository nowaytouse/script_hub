use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

const MAX_LOG_LINES: usize = 1_000;
const MAX_LOG_BYTES: usize = 512 * 1024;
const MAX_LINE_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobKind {
    Validate,
    RefreshCatalog,
    Update,
    QuickUpdate,
}

impl JobKind {
    fn is_mutating(self) -> bool {
        !matches!(self, Self::Validate)
    }
}

#[derive(Clone, Debug)]
pub struct JobOptions {
    pub root: PathBuf,
    pub singbox: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub running: bool,
    pub status: String,
    pub logs: VecDeque<String>,
}

struct State {
    running: bool,
    status: String,
    logs: VecDeque<String>,
    log_bytes: usize,
    has_warnings: bool,
}

impl State {
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
    pub fn start(kind: JobKind, options: JobOptions) -> Result<Self, String> {
        let root = validate_root(&options.root)?;
        let singbox = match (kind, options.singbox) {
            (JobKind::Update | JobKind::QuickUpdate, Some(path)) => {
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
        if matches!(kind, JobKind::Update | JobKind::QuickUpdate) {
            command.arg("--execute");
        }
        if kind == JobKind::QuickUpdate {
            command.arg("--quick");
        }

        let state = Arc::new(Mutex::new(State {
            running: true,
            status: match kind {
                JobKind::Validate => "校验正在运行".to_string(),
                JobKind::RefreshCatalog => "目录刷新正在运行".to_string(),
                JobKind::Update => "全量更新正在运行".to_string(),
                JobKind::QuickUpdate => "快速更新正在运行".to_string(),
            },
            logs: VecDeque::new(),
            log_bytes: 0,
            has_warnings: false,
        }));
        let worker_state = Arc::clone(&state);
        let worker = thread::Builder::new()
            .name("script-hub-gui-job".to_string())
            .spawn(move || {
                let result = run_worker(command, Arc::clone(&worker_state));
                let mut state = worker_state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.status = match result {
                    Ok(()) if state.has_warnings => "已完成（有警告）".to_string(),
                    Ok(()) => "已完成".to_string(),
                    Err(error) => {
                        state.push(format!("[GUI ERROR] {error}"));
                        "失败".to_string()
                    }
                };
                state.running = false;
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
    }
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
            .spawn(move || drain(stdout, out_state))
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
            .spawn(move || drain(stderr, err_state))
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

fn drain(mut reader: impl Read, state: Arc<Mutex<State>>) -> Result<(), String> {
    let mut buffer = [0; 4096];
    let mut line = Vec::with_capacity(256);
    let mut truncated = false;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("cannot read worker output: {error}"))?;
        if count == 0 {
            if !line.is_empty() || truncated {
                push_line(&state, &line, truncated);
            }
            return Ok(());
        }
        for byte in &buffer[..count] {
            if *byte == b'\n' {
                push_line(&state, &line, truncated);
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

fn push_line(state: &Arc<Mutex<State>>, bytes: &[u8], truncated: bool) {
    let mut line = String::from_utf8_lossy(bytes)
        .trim_end_matches('\r')
        .to_string();
    if truncated {
        line.push_str(" …[行已截断]");
    }
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(line);
}

#[cfg(test)]
mod tests {
    use super::{
        JobKind, JobOptions, MAX_LOG_BYTES, State, acquire_write_lock, drain, run_worker,
        validate_root,
    };
    use std::collections::VecDeque;
    use std::io::{self, Read};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn log_colors_are_removed_without_damaging_unicode() {
        assert_eq!(
            super::plain_log_line("\u{1b}[0;34m[INFO]\u{1b}[0m 中文\r"),
            "[INFO] 中文"
        );
    }

    #[test]
    fn log_buffer_stays_within_line_and_byte_limits() {
        let mut state = State {
            running: true,
            status: String::new(),
            logs: VecDeque::new(),
            log_bytes: 0,
            has_warnings: false,
        };
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
    fn output_reader_errors_are_returned() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("fixture read failure"))
            }
        }

        let state = Arc::new(Mutex::new(State {
            running: true,
            status: String::new(),
            logs: VecDeque::new(),
            log_bytes: 0,
            has_warnings: false,
        }));
        assert!(
            drain(FailingReader, state)
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
        let state = Arc::new(Mutex::new(State {
            running: true,
            status: String::new(),
            logs: VecDeque::new(),
            log_bytes: 0,
            has_warnings: false,
        }));
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
        let _options = JobOptions {
            root: Path::new(".").to_path_buf(),
            singbox: None,
        };
    }
}
