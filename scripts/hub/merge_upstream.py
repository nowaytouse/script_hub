#!/usr/bin/env python3
"""Download and merge upstream Surge modules into one bundle (deduped, sanitized)."""

from __future__ import annotations

import os
from datetime import datetime
from typing import Dict, List, Optional, Sequence, Tuple

from hub.common import Logger, read_file, safe_download
from hub.module_sanitizer import (
    format_header,
    format_module,
    merge_mitm_hosts,
    parse_module,
)

SourceSpec = Tuple[str, str]  # (label, url)


def _fetch_module(
    url: str,
    label: str,
    *,
    local_fallback: Optional[str] = None,
) -> str:
    if url.startswith(("http://", "https://")):
        content = safe_download(url, retries=2, timeout=60)
        if content and len(content.strip()) >= 20:
            return content
    elif os.path.isfile(url):
        text = "".join(read_file(url))
        if len(text.strip()) >= 20:
            return text
    if local_fallback and os.path.isfile(local_fallback):
        Logger.warn(f"Using local fallback for {label}")
        return "".join(read_file(local_fallback))
    raise RuntimeError(f"Failed to load upstream module: {label} ({url})")


def _combine_arguments(
    parts: Sequence[Tuple[str, Dict[str, str]]],
) -> Tuple[Optional[str], Optional[str]]:
    arg_tokens: List[str] = []
    desc_blocks: List[str] = []

    for label, meta in parts:
        raw_args = meta.get("arguments", "").strip()
        if raw_args:
            for token in raw_args.split(","):
                token = token.strip()
                if token and token not in arg_tokens:
                    arg_tokens.append(token)

        raw_desc = meta.get("arguments-desc", "").strip()
        mod_name = meta.get("name", label).strip()
        if raw_desc:
            # iOS clients are sensitive to oversized metadata lines. Keep concise summaries only.
            desc_blocks.append(f"[{mod_name}] 参数说明请参考上游模块文档")

    args_line = ",".join(arg_tokens) if arg_tokens else None
    desc_line = "\\n".join(desc_blocks) if desc_blocks else None
    return args_line, desc_line


def merge_upstream_modules(
    sources: Sequence[SourceSpec],
    output_path: str,
    *,
    header_meta: Dict[str, str],
    provenance_comment: Optional[str] = None,
    optional_labels: Optional[Sequence[str]] = None,
    local_fallbacks: Optional[Dict[str, str]] = None,
) -> None:
    """Merge remote modules into output_path using module_sanitizer."""
    optional = set(optional_labels or ())
    fallbacks = local_fallbacks or {}
    parsed_parts: List[Tuple[str, Dict[str, str], List]] = []
    merged_sections: Dict[str, List[str]] = {}
    used_sources: List[SourceSpec] = []

    for label, url in sources:
        Logger.info(f"Downloading {label}...")
        try:
            text = _fetch_module(url, label, local_fallback=fallbacks.get(label))
        except RuntimeError as exc:
            if label in optional:
                Logger.warn(f"Skipping optional source {label}: {exc}")
                continue
            raise
        meta, sections = parse_module(text)
        parsed_parts.append((label, meta, sections))
        used_sources.append((label, url))
        for name, lines in sections:
            merged_sections.setdefault(name, []).extend(lines)

    if not parsed_parts:
        raise RuntimeError(f"No upstream modules downloaded for {output_path}")

    combined_args, combined_desc = _combine_arguments(
        [(label, meta) for label, meta, _ in parsed_parts]
    )

    out_meta = dict(header_meta)
    out_meta["date"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    if combined_args:
        out_meta["arguments"] = combined_args
    if combined_desc:
        out_meta["arguments-desc"] = combined_desc

    section_list = [(name, merged_sections[name]) for name in merged_sections]
    section_list = merge_mitm_hosts(section_list)

    extra: List[str] = []
    if provenance_comment:
        extra.append(provenance_comment)
    extra.append("# Upstream modules merged by scripts/hub/merge_upstream.py")
    for label, url in used_sources:
        extra.append(f"# - {label}: {url}")

    header_lines = format_header(out_meta, extra_lines=extra)
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        f.write(format_module(header_lines, section_list, dedupe=True))

    _validate_merged_module(output_path)
    Logger.success(f"Wrote bundle: {output_path}")


def _validate_merged_module(output_path: str) -> None:
    """Reject merged bundles with known Surge-breaking merge artifacts."""
    text = "".join(read_file(output_path))
    if "%INSERT%" in text:
        raise RuntimeError(
            f"Merged module contains invalid MITM token %INSERT%: {output_path}"
        )
    if "hostname = %APPEND% %INSERT%" in text or "hostname = %INSERT%" in text:
        raise RuntimeError(f"Merged MITM hostname line is invalid: {output_path}")
