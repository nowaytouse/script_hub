//! Optional, privacy-conscious history for completed desktop jobs.
use rusqlite::{Connection, OpenFlags, params};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SCHEMA_VERSION: i64 = 1;
const RETAINED_RECORDS: i64 = 100;
const MAX_SAVED_LOG_LINES: usize = 200;
const MAX_SAVED_LOG_BYTES: usize = 64 * 1024;
const MAX_SAVED_LOG_JSON_BYTES: i64 = 128 * 1024;

#[derive(Clone, Debug)]
pub struct HistoryOptions {
    pub path: PathBuf,
    pub keep_logs: bool,
}

#[derive(Clone, Debug)]
pub struct HistoryRecord {
    pub id: i64,
    pub kind: String,
    pub started_at: i64,
    pub elapsed_ms: u64,
    pub outcome: String,
    pub exit_code: Option<i32>,
    pub logs: Vec<String>,
}

pub struct HistoryStore {
    connection: Connection,
}

impl HistoryStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = prepare_database_file(path)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let connection = Connection::open_with_flags(&path, flags).map_err(|error| {
            format!(
                "cannot open task history database {}: {error}",
                path.display()
            )
        })?;
        connection
            .busy_timeout(Duration::from_secs(3))
            .map_err(|error| format!("cannot configure task history database: {error}"))?;
        let store = Self { connection };
        store.initialize()?;
        Ok(store)
    }

    fn initialize(&self) -> Result<(), String> {
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| format!("cannot read task history schema version: {error}"))?;
        match version {
            0 => {
                let objects: i64 = self
                    .connection
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|error| format!("cannot inspect task history schema: {error}"))?;
                if objects != 0 {
                    return Err(
                        "task history database has an unversioned or malformed schema".to_string(),
                    );
                }
                let transaction = self.connection.unchecked_transaction().map_err(|error| {
                    format!("cannot begin task history schema transaction: {error}")
                })?;
                transaction.execute_batch(
                    "CREATE TABLE history (
                        id INTEGER PRIMARY KEY,
                        kind TEXT NOT NULL,
                        started_at INTEGER NOT NULL,
                        elapsed_ms INTEGER NOT NULL DEFAULT 0,
                        outcome TEXT NOT NULL CHECK(outcome IN ('running','success','warnings','failed')),
                        exit_code INTEGER,
                        logs_json TEXT NOT NULL DEFAULT '[]'
                    );
                    PRAGMA user_version = 1;",
                ).map_err(|error| format!("cannot initialize task history schema: {error}"))?;
                transaction
                    .commit()
                    .map_err(|error| format!("cannot commit task history schema: {error}"))?;
            }
            SCHEMA_VERSION => {
                self.connection.prepare(
                    "SELECT id, kind, started_at, elapsed_ms, outcome, exit_code, logs_json FROM history LIMIT 0",
                ).map_err(|error| format!("task history schema is malformed: {error}"))?;
            }
            future => {
                return Err(format!(
                    "task history schema version {future} is newer than supported version {SCHEMA_VERSION}"
                ));
            }
        }
        Ok(())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<HistoryRecord>, String> {
        let limit = limit.min(RETAINED_RECORDS as usize) as i64;
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, kind, started_at, elapsed_ms, outcome, exit_code, length(CAST(logs_json AS BLOB)), logs_json
             FROM history ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|error| format!("cannot query task history: {error}"))?;
        let rows = statement
            .query_map([limit], |row| {
                let logs_size: i64 = row.get(6)?;
                if !(0..=MAX_SAVED_LOG_JSON_BYTES).contains(&logs_size) {
                    return Err(rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "saved task logs exceed the history read limit",
                        )),
                    ));
                }
                let logs_json: String = row.get(7)?;
                let logs = serde_json::from_str(&logs_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        logs_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(HistoryRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    started_at: row.get(2)?,
                    elapsed_ms: u64::try_from(row.get::<_, i64>(3)?.max(0)).unwrap_or(u64::MAX),
                    outcome: row.get(4)?,
                    exit_code: row.get(5)?,
                    logs,
                })
            })
            .map_err(|error| format!("cannot read task history: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("cannot decode task history: {error}"))
    }

    pub fn clear(&self) -> Result<(), String> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| format!("cannot begin task history clear: {error}"))?;
        transaction
            .execute("DELETE FROM history WHERE outcome <> 'running'", [])
            .map_err(|error| format!("cannot clear task history: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("cannot commit task history clear: {error}"))
    }

    /// Remove unfinished rows only after the user explicitly confirms recovery.
    /// Callers must not infer that a row is stale; another app instance may own it.
    pub fn clear_unfinished(&self) -> Result<(), String> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| format!("cannot begin unfinished history recovery: {error}"))?;
        transaction
            .execute("DELETE FROM history WHERE outcome = 'running'", [])
            .map_err(|error| format!("cannot remove unfinished task history: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("cannot commit unfinished history recovery: {error}"))
    }

    pub fn start(&self, kind: &str) -> Result<i64, String> {
        if kind.trim().is_empty() {
            return Err("task history kind must not be empty".to_string());
        }
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| format!("cannot begin task history start: {error}"))?;
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .map_err(|error| format!("cannot count task history records: {error}"))?;
        let excess = count.saturating_add(1).saturating_sub(RETAINED_RECORDS);
        if excess > 0 {
            let removed = transaction
                .execute(
                    "DELETE FROM history WHERE id IN (
                        SELECT id FROM history WHERE outcome <> 'running' ORDER BY id ASC LIMIT ?1
                    )",
                    [excess],
                )
                .map_err(|error| format!("cannot reserve task history capacity: {error}"))?;
            if i64::try_from(removed).unwrap_or(i64::MAX) < excess {
                return Err(
                    "task history is full of unfinished jobs; cannot record another task"
                        .to_string(),
                );
            }
        }
        transaction
            .execute(
                "INSERT INTO history (kind, started_at, outcome) VALUES (?1, ?2, 'running')",
                params![kind, unix_seconds()],
            )
            .map_err(|error| format!("cannot record task history start: {error}"))?;
        let id = transaction.last_insert_rowid();
        transaction
            .commit()
            .map_err(|error| format!("cannot commit task history start: {error}"))?;
        Ok(id)
    }

    pub fn finish(
        &self,
        id: i64,
        snapshot: &super::job::Snapshot,
        keep_logs: bool,
    ) -> Result<(), String> {
        let (outcome, exit_code) = match snapshot.outcome {
            Some(super::job::JobOutcome::Success) => ("success", snapshot.exit_code),
            Some(super::job::JobOutcome::Warnings) => ("warnings", snapshot.exit_code),
            Some(super::job::JobOutcome::Failed) => ("failed", snapshot.exit_code),
            None => return Err("cannot finish task history without a final outcome".to_string()),
        };
        let logs = if keep_logs {
            bounded_redacted_logs(snapshot.logs.iter().map(String::as_str))
        } else {
            Vec::new()
        };
        let logs_json = serde_json::to_string(&logs)
            .map_err(|error| format!("cannot encode task history logs: {error}"))?;
        let elapsed_ms = i64::try_from(snapshot.elapsed.as_millis()).unwrap_or(i64::MAX);
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| format!("cannot begin task history finish: {error}"))?;
        let changed = transaction
            .execute(
                "UPDATE history SET elapsed_ms = ?1, outcome = ?2, exit_code = ?3, logs_json = ?4
             WHERE id = ?5 AND outcome = 'running'",
                params![elapsed_ms, outcome, exit_code, logs_json, id],
            )
            .map_err(|error| format!("cannot record task history finish: {error}"))?;
        if changed != 1 {
            return Err(format!(
                "task history record {id} is missing or already finished"
            ));
        }
        transaction.execute(
            "DELETE FROM history WHERE outcome <> 'running' AND id NOT IN (SELECT id FROM history WHERE outcome <> 'running' ORDER BY id DESC LIMIT ?1)",
            [RETAINED_RECORDS],
        ).map_err(|error| format!("cannot prune task history: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("cannot commit task history finish: {error}"))
    }
}

fn prepare_database_file(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create task history directory {}: {error}",
            parent.display()
        )
    })?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!(
                "task history path is not a regular file: {}",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let created = private_create_options().open(path).map_err(|error| {
                format!(
                    "cannot create task history database {}: {error}",
                    path.display()
                )
            })?;
            drop(created);
        }
        Err(error) => {
            return Err(format!(
                "cannot inspect task history path {}: {error}",
                path.display()
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!("cannot protect task history database permissions: {error}")
        })?;
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("task history path has no filename: {}", path.display()))?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        format!(
            "cannot resolve task history directory {}: {error}",
            parent.display()
        )
    })?;
    Ok(canonical_parent.join(file_name))
}

#[cfg(unix)]
fn private_create_options() -> OpenOptions {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    options
}

#[cfg(not(unix))]
fn private_create_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    options
}

fn unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn bounded_redacted_logs<'a, I>(logs: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
    I::IntoIter: DoubleEndedIterator,
{
    let mut saved = Vec::new();
    let mut bytes = 0;
    // Retain the newest diagnostics first; final errors must survive both limits.
    for line in logs.into_iter().rev().take(MAX_SAVED_LOG_LINES) {
        let redacted = redact_log_line(line);
        let remaining = MAX_SAVED_LOG_BYTES.saturating_sub(bytes);
        if remaining == 0 {
            break;
        }
        let mut bounded = String::new();
        for character in redacted.chars() {
            if bounded.len() + character.len_utf8() > remaining {
                break;
            }
            bounded.push(character);
        }
        bytes += bounded.len();
        saved.push(bounded);
        if bytes >= MAX_SAVED_LOG_BYTES {
            break;
        }
    }
    saved.reverse();
    saved
}

// shortcut: heuristic redaction can miss unknown secret formats; add patterns when a new format appears.
fn redact_log_line(line: &str) -> String {
    use std::sync::OnceLock;
    static ASSIGNMENT: OnceLock<regex::Regex> = OnceLock::new();
    static AUTHORIZATION: OnceLock<regex::Regex> = OnceLock::new();
    static BEARER: OnceLock<regex::Regex> = OnceLock::new();
    static URL: OnceLock<regex::Regex> = OnceLock::new();
    let assignments = ASSIGNMENT.get_or_init(|| regex::Regex::new(
        r#"(?i)\b(password|passwd|token|access[_-]?token|refresh[_-]?token|api[_-]?key|secret|client[_-]?secret|private[_-]?key|credential|session[_-]?id)(\s*[:=]\s*)(["']?)([^\s"'&,;]+)"#
    ).expect("constant redaction regex"));
    let authorization = AUTHORIZATION.get_or_init(|| {
        regex::Regex::new(r"(?i)\bAuthorization\s*:\s*[^\r\n]+")
            .expect("constant authorization regex")
    });
    let bearer = BEARER.get_or_init(|| {
        regex::Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+").expect("constant bearer regex")
    });
    let urls = URL.get_or_init(|| {
        regex::Regex::new(r#"(?i)\b(?:https?|ftp)://[^\s"'<>]+"#).expect("constant URL regex")
    });
    let scrubbed = authorization.replace_all(line, "Authorization: [REDACTED]");
    let scrubbed = assignments.replace_all(&scrubbed, "$1$2[REDACTED]");
    let scrubbed = bearer.replace_all(&scrubbed, "Bearer [REDACTED]");
    urls.replace_all(&scrubbed, |captures: &regex::Captures<'_>| {
        let value = captures.get(0).map_or("", |capture| capture.as_str());
        let punctuation = value
            .chars()
            .rev()
            .take_while(|character| ".,;)]}".contains(*character))
            .collect::<String>();
        let url_end = value.len() - punctuation.len();
        let url = &value[..url_end];
        let Some((scheme, rest)) = url.split_once("://") else {
            return value.to_string();
        };
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let authority = rest[..authority_end]
            .rsplit('@')
            .next()
            .unwrap_or(&rest[..authority_end]);
        let path = &rest[authority_end..];
        let path_end = path.find(['?', '#']).unwrap_or(path.len());
        let punctuation: String = punctuation.chars().rev().collect();
        format!("{scheme}://{}{}{punctuation}", authority, &path[..path_end])
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::{HistoryStore, bounded_redacted_logs, redact_log_line};
    use crate::desktop::job::{JobOutcome, Snapshot};
    use std::collections::VecDeque;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("script-hub-history-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn snapshot(outcome: JobOutcome, logs: &[&str]) -> Snapshot {
        Snapshot {
            running: false,
            status: String::new(),
            logs: logs
                .iter()
                .map(|line| (*line).to_string())
                .collect::<VecDeque<_>>(),
            stage: None,
            progress: None,
            outcome: Some(outcome),
            elapsed: Duration::from_millis(123),
            exit_code: Some(0),
        }
    }

    fn database(directory: &TestDir) -> PathBuf {
        directory.path().join("history.db")
    }

    #[test]
    fn open_creates_missing_parent_directories() {
        let dir = TestDir::new();
        let path = dir.path().join("support/history/history.sqlite3");
        let store = HistoryStore::open(&path).unwrap();
        assert!(path.is_file());
        assert!(store.recent(usize::MAX).unwrap().is_empty());
    }

    #[test]
    fn records_survive_reopen_and_summary_mode_drops_logs() {
        let dir = TestDir::new();
        let path = database(&dir);
        let id = {
            let store = HistoryStore::open(&path).unwrap();
            let id = store.start("update").unwrap();
            store
                .finish(id, &snapshot(JobOutcome::Success, &["token=hidden"]), false)
                .unwrap();
            id
        };
        let store = HistoryStore::open(&path).unwrap();
        let record = store.recent(10).unwrap().pop().unwrap();
        assert_eq!(record.id, id);
        assert_eq!(record.outcome, "success");
        assert!(record.logs.is_empty());
        store.clear().unwrap();
        assert!(store.recent(10).unwrap().is_empty());
    }

    #[test]
    fn rejects_malformed_and_future_schema_without_replacing_file() {
        let dir = TestDir::new();
        let path = database(&dir);
        fs::write(&path, "not a sqlite database").unwrap();
        assert!(HistoryStore::open(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"not a sqlite database");

        let malformed = dir.path().join("malformed.db");
        let connection = rusqlite::Connection::open(&malformed).unwrap();
        connection
            .execute_batch("CREATE TABLE history (wrong TEXT); PRAGMA user_version = 1;")
            .unwrap();
        drop(connection);
        assert!(
            HistoryStore::open(&malformed)
                .err()
                .unwrap()
                .contains("malformed")
        );

        let future = dir.path().join("future.db");
        let store = HistoryStore::open(&future).unwrap();
        store
            .connection
            .pragma_update(None, "user_version", 99)
            .unwrap();
        drop(store);
        assert!(HistoryStore::open(&future).err().unwrap().contains("newer"));
    }

    #[test]
    fn clear_and_retention_preserve_running_rows() {
        let dir = TestDir::new();
        let store = HistoryStore::open(&database(&dir)).unwrap();
        let running_id = store.start("update").unwrap();
        for _ in 0..101 {
            let id = store.start("validate").unwrap();
            store
                .finish(id, &snapshot(JobOutcome::Success, &[]), false)
                .unwrap();
        }
        assert!(
            store
                .recent(200)
                .unwrap()
                .iter()
                .any(|record| record.id == running_id)
        );
        store.clear().unwrap();
        let records = store.recent(200).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].outcome, "running");
    }

    #[test]
    fn finish_and_retention_rollback_together_on_sql_error() {
        let dir = TestDir::new();
        let store = HistoryStore::open(&database(&dir)).unwrap();
        let mut ids = Vec::new();
        for _ in 0..100 {
            ids.push(store.start("validate").unwrap());
        }
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_finish BEFORE UPDATE ON history
             BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
            )
            .unwrap();
        assert!(
            store
                .finish(
                    *ids.last().unwrap(),
                    &snapshot(JobOutcome::Success, &[]),
                    false
                )
                .is_err()
        );
        let running = store
            .connection
            .query_row(
                "SELECT outcome FROM history WHERE id = ?1",
                [ids[99]],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        let count: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(running, "running");
        assert_eq!(count, 100);
    }

    #[test]
    fn finish_rejects_missing_or_already_finished_id() {
        let dir = TestDir::new();
        let store = HistoryStore::open(&database(&dir)).unwrap();
        let id = store.start("validate").unwrap();
        store
            .finish(id, &snapshot(JobOutcome::Success, &[]), false)
            .unwrap();
        assert!(
            store
                .finish(id, &snapshot(JobOutcome::Success, &[]), false)
                .is_err()
        );
        assert!(
            store
                .finish(i64::MAX, &snapshot(JobOutcome::Success, &[]), false)
                .is_err()
        );
    }

    #[test]
    fn normal_clear_preserves_unfinished_rows_for_separate_recovery() {
        let dir = TestDir::new();
        let store = HistoryStore::open(&database(&dir)).unwrap();
        for _ in 0..100 {
            store.start("update").unwrap();
        }
        assert!(store.start("update").is_err());
        let records = store.recent(usize::MAX).unwrap();
        assert_eq!(records.len(), 100);
        assert!(records.iter().all(|record| record.outcome == "running"));
        store.clear().unwrap();
        assert_eq!(store.recent(usize::MAX).unwrap().len(), 100);
    }

    #[test]
    fn explicit_unfinished_recovery_frees_capacity_and_preserves_completed_rows() {
        let dir = TestDir::new();
        let store = HistoryStore::open(&database(&dir)).unwrap();
        let completed_id = store.start("validate").unwrap();
        store
            .finish(completed_id, &snapshot(JobOutcome::Success, &[]), false)
            .unwrap();
        for _ in 0..99 {
            store.start("update").unwrap();
        }

        assert_eq!(store.recent(usize::MAX).unwrap().len(), 100);
        store.clear_unfinished().unwrap();
        let records = store.recent(usize::MAX).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, completed_id);
        assert_eq!(records[0].outcome, "success");

        store.start("quick_update").unwrap();
        assert_eq!(store.recent(usize::MAX).unwrap().len(), 2);
    }

    #[test]
    fn capacity_reservation_prunes_completed_and_rolls_back_on_failure() {
        let dir = TestDir::new();
        let store = HistoryStore::open(&database(&dir)).unwrap();
        let completed = store.start("validate").unwrap();
        store
            .finish(completed, &snapshot(JobOutcome::Success, &[]), false)
            .unwrap();
        for _ in 0..99 {
            store.start("update").unwrap();
        }
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_prune BEFORE DELETE ON history
             BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
            )
            .unwrap();
        assert!(store.start("update").is_err());
        let count: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 100);
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT outcome FROM history WHERE id = ?1",
                    [completed],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "success"
        );
        store
            .connection
            .execute_batch("DROP TRIGGER reject_prune;")
            .unwrap();
        store.start("update").unwrap();
        let count: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 100);
        assert!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM history WHERE outcome = 'running'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap()
                == 100
        );
    }

    #[test]
    fn keeps_only_latest_one_hundred_after_finish() {
        let dir = TestDir::new();
        let store = HistoryStore::open(&database(&dir)).unwrap();
        for _ in 0..101 {
            let id = store.start("validate").unwrap();
            store
                .finish(id, &snapshot(JobOutcome::Success, &[]), false)
                .unwrap();
        }
        let records = store.recent(200).unwrap();
        assert_eq!(records.len(), 100);
        assert_eq!(records.first().unwrap().id, 101);
    }

    #[test]
    fn rejects_oversized_persisted_log_payloads_before_decoding() {
        let dir = TestDir::new();
        let store = HistoryStore::open(&database(&dir)).unwrap();
        let id = store.start("validate").unwrap();
        store
            .finish(id, &snapshot(JobOutcome::Success, &[]), false)
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE history SET logs_json = zeroblob(?1) WHERE id = ?2",
                rusqlite::params![super::MAX_SAVED_LOG_JSON_BYTES + 1, id],
            )
            .unwrap();
        assert!(store.recent(1).is_err());
    }

    #[test]
    fn bounds_and_redacts_opted_in_logs() {
        let logs = bounded_redacted_logs([
            &"x".repeat(70 * 1024),
            "GET https://user:pass@example.test?token=secret#private token=abc",
        ]);
        assert_eq!(logs.len(), 2);
        assert!(!logs[1].contains("pass"));
        assert!(!logs[1].contains("secret"));
        assert!(!logs[1].contains("private"));
        assert!(!logs[1].contains("abc"));
        assert!(logs.iter().map(String::len).sum::<usize>() <= 64 * 1024);
        assert_eq!(
            redact_log_line("Authorization: Bearer abc.def"),
            "Authorization: [REDACTED]"
        );
        let noisy: Vec<String> = (0..250).map(|index| format!("line {index}")).collect();
        let bounded = bounded_redacted_logs(noisy.iter().map(String::as_str));
        assert_eq!(bounded.len(), 200);
        assert_eq!(bounded.first().map(String::as_str), Some("line 50"));
        assert_eq!(bounded.last().map(String::as_str), Some("line 249"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_database_path_and_restricts_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = TestDir::new();
        let target = database(&dir);
        let store = HistoryStore::open(&target).unwrap();
        drop(store);
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let link = dir.path().join("linked.db");
        symlink(&target, &link).unwrap();
        assert!(HistoryStore::open(&link).is_err());
    }
}
