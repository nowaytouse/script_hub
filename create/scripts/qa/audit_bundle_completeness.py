#!/usr/bin/env python3
"""Audit merged Surge bundle modules for merge regressions (MITM, script dedupe)."""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS_DIR))

from hub.common import Logger, get_project_root
from hub.merge_upstream import _fetch_module
from hub.module_sanitizer import parse_module

SCRIPT_LABEL_RE = re.compile(r"^(.+?)\s*=\s*type=", re.I)
MITM_BAD_TOKENS = frozenset({"%INSERT%", "%APPEND%"})


@dataclass
class BundleSpec:
    label: str
    output: Path
    sources: Sequence[tuple[str, str]]
    required_script_labels: Sequence[str] = ()
    required_mitm_hosts: Sequence[str] = ()
    required_script_path_substrings: Sequence[str] = ()


def _script_labels(sections: list[tuple[str, list[str]]]) -> set[str]:
    labels: set[str] = set()
    for name, lines in sections:
        if name != "Script":
            continue
        for line in lines:
            m = SCRIPT_LABEL_RE.match(line.strip())
            if m:
                labels.add(m.group(1).strip())
    return labels


def _mitm_hosts(sections: list[tuple[str, list[str]]]) -> set[str]:
    hosts: set[str] = set()
    for name, lines in sections:
        if name != "MITM":
            continue
        for line in lines:
            if not line.strip().lower().startswith("hostname"):
                continue
            part = re.sub(r"^hostname\s*=\s*", "", line.strip(), flags=re.I)
            part = re.sub(r"^(%APPEND%|%INSERT%)\s*", "", part, flags=re.I)
            for token in part.split(","):
                token = token.strip()
                if token and token not in MITM_BAD_TOKENS:
                    hosts.add(token)
    return hosts


def audit_bundle(spec: BundleSpec) -> list[str]:
    issues: list[str] = []
    if not spec.output.is_file():
        return [f"{spec.label}: bundle missing at {spec.output}"]

    bundle_text = spec.output.read_text(encoding="utf-8")
    _, bundle_sections = parse_module(bundle_text)
    bundle_scripts = _script_labels(bundle_sections)
    bundle_mitm = _mitm_hosts(bundle_sections)

    if "%INSERT%" in bundle_text:
        issues.append(f"{spec.label}: contains invalid MITM token %INSERT%")

    for needle in spec.required_script_path_substrings:
        if needle not in bundle_text:
            issues.append(f"{spec.label}: vendored script-path missing: {needle}")

    upstream_scripts: set[str] = set()
    upstream_mitm: set[str] = set()

    for src_label, url in spec.sources:
        try:
            text = _fetch_module(url, src_label)
        except RuntimeError as exc:
            issues.append(f"{spec.label}: failed to fetch upstream [{src_label}]: {exc}")
            continue
        _, sections = parse_module(text)
        upstream_scripts |= _script_labels(sections)
        upstream_mitm |= _mitm_hosts(sections)

    missing_scripts = upstream_scripts - bundle_scripts
    if missing_scripts:
        issues.append(
            f"{spec.label}: missing script label(s): {', '.join(sorted(missing_scripts)[:12])}"
            + ("…" if len(missing_scripts) > 12 else "")
        )

    for req in spec.required_script_labels:
        if req not in bundle_scripts:
            issues.append(f"{spec.label}: required script missing: {req}")

    missing_mitm_positive = {
        h for h in (upstream_mitm - bundle_mitm) if not h.startswith("-")
    }
    for req in spec.required_mitm_hosts:
        if req not in bundle_mitm:
            missing_mitm_positive.add(req)
    if missing_mitm_positive:
        issues.append(
            f"{spec.label}: MITM missing host(s): {', '.join(sorted(missing_mitm_positive))}"
        )

    return issues


def _specs(root: Path) -> list[BundleSpec]:
    from pipeline import merge_bundles as mb

    return [
        BundleSpec(
            "BiliBili",
            Path(mb.BILIBILI_OUTPUT),
            mb.BILIBILI_SOURCES,
            required_mitm_hosts=("app.bilibili.com",),
        ),
        BundleSpec(
            "Apple",
            Path(mb.APPLE_OUTPUT),
            mb.APPLE_SOURCES,
        ),
        BundleSpec(
            "Panel utilities",
            Path(mb.UTILITIES_OUTPUT),
            mb.UTILITIES_SOURCES,
            required_mitm_hosts=("net-lsp-x.com",),
        ),
        BundleSpec(
            "Script Hub devtools",
            Path(mb.DEVTOOLS_OUTPUT),
            mb.DEVTOOLS_SOURCES,
            required_script_labels=(
                "Sub-Store Core",
                "Sub-Store Simple",
                "{{{sync}}}",
                "{{{produce}}}",
                "Rewrite: BoxJs",
                "Script Hub: 前端",
            ),
            required_mitm_hosts=("sub.store", "script.hub", "boxjs.com"),
            required_script_path_substrings=(
                "modules/source/scripts/github_com_sub-store-org_sub-store-1.min.js",
                "modules/source/scripts/raw_githubusercontent_com_f59ef7_script-hub.js",
            ),
        ),
        BundleSpec(
            "Weibo (PROMAX source)",
            Path(mb.WEIBO_LOCAL),
            mb.WEIBO_SOURCES,
        ),
    ]


def audit_promax_modules(root: Path) -> list[str]:
    issues: list[str] = []
    for rel in (
        "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
    ):
        path = root / rel
        if not path.is_file():
            issues.append(f"PROMAX: missing {rel}")
            continue
        text = path.read_text(encoding="utf-8")
        if "%INSERT%" in text:
            issues.append(f"PROMAX: %INSERT% found in {path.name}")
    return issues


def main() -> int:
    root = Path(get_project_root())
    specs = _specs(root)
    Logger.section("Bundle completeness audit")
    all_issues: list[str] = []
    for spec in specs:
        Logger.info(f"Checking {spec.label}…")
        all_issues.extend(audit_bundle(spec))
    all_issues.extend(audit_promax_modules(root))

    if all_issues:
        Logger.error(f"Found {len(all_issues)} issue(s):")
        for item in all_issues:
            print(f"  • {item}")
        return 1

    Logger.success(f"All {len(specs)} bundle(s) + PROMAX passed completeness audit.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
