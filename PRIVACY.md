# Privacy

Script Hub is a local desktop and command-line tool. It does not automatically upload workspace files, settings, task history, or diagnostic logs.

## Local data

- Application preferences are stored in the platform-specific settings JSON file documented in the README.
- The desktop app stores task-history summaries in a local SQLite database and keeps up to 100 records by default. This history is local application data, not a cloud account or synchronization service.
- Diagnostic logging is opt-in. Logs are bounded and redacted, but redaction is not a guarantee that every secret or sensitive value is removed. Review logs before copying, attaching, or sharing them.
- The application does not automatically scan workspaces at startup by default. A workspace is read or written only after the user opens it or explicitly starts an action that operates on it.

## Network activity

Network access is initiated only by an explicit action such as updating project sources, refreshing a configured custom source, or a manually requested Git operation. These actions contact the configured upstream services and Git remotes. External scripts referenced by project configuration may be fetched and executed by the user's client at runtime; those requests are made by that client, outside Script Hub's local processing.

Review the action, selected workspace, source, and destination before confirming. Review generated files and logs before exporting or sharing them. Script Hub does not promise that upstream services retain no connection metadata.

## Retention and deletion

Settings and task-history summaries remain in local application data until changed or removed by the user or the operating system. Diagnostic logging is opt-in and bounded; its retention depends on the selected output and is not an upload mechanism. This document does not claim a secure-erasure mechanism. The task-history default limit is 100 records; changing application settings or removing local data can affect the app's remembered state.

This notice describes the current application behavior, not a legal privacy guarantee. Report suspected privacy or security issues using the process in [SECURITY.md](SECURITY.md).
