# Maintenance

Keep READMEs minimal. Record user-facing release changes in the English CHANGELOG.md only; routine generated-data updates do not need prose changes.

Public sample templates remain ciphertext and require manual password entry. Never publish plaintext configurations, private libraries, subscriptions or tokens. The documented sample password is intentionally public and must not protect private data.

Work on master. Do not create branches or rewrite history without explicit user authorization. Preserve unrelated work and recoverable backups.

For code navigation, a private index may be available at `${XDG_STATE_HOME:-~/.local/state}/project-indexes/<checkout-basename>/latest.json`. Verify its full Git HEAD and timestamp before use; rebuild from the current committed source when stale. Keep index output private and uncommitted. Use synthetic fixtures for privacy and QA; exclude vaults, logs, and real dumps.
