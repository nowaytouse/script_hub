# Module Header Spec (Locked Rules)

This spec prevents the iOS "module fields empty" regression.

## Hard Rules

- Header lines before the first section MUST be only:
  - metadata lines: `#!key=value`
  - comments: `# ...`
  - blank lines
- Any non-comment plain text before the first section is invalid.
- Every module MUST contain at least one section like `[Rule]`, `[Script]`, `[MITM]`, etc.
- Recommended metadata keys:
  - `name`
  - `desc`
  - `author`
  - `category`

## Arguments Metadata Safety

- `#!arguments-desc` MUST remain a single metadata line.
- Do not emit raw multi-line body text in header area.
- If multi-line meaning is needed, use escaped newlines (`\n`) in metadata value.
- Keep metadata compact to avoid client parser edge-cases.

## Enforced By

- Local/CI validator: `scripts/tools/validate_module_headers.py`
- CI workflow gate: `.github/workflows/update_rulesets.yml`
- Main pipeline gate: `scripts/main_update.py`

