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


def _combine_metadata(
    parts: Sequence[Tuple[str, Dict[str, str]]],
) -> Tuple[Optional[str], Optional[str], Optional[str]]:
    arg_tokens: List[str] = []
    args_desc_blocks: List[str] = []
    desc_blocks: List[str] = []

    for label, meta in parts:
        mod_name = meta.get("name", label).strip()

        # Handle arguments
        raw_args = meta.get("arguments", "").strip()
        if raw_args:
            for token in raw_args.split(","):
                token = token.strip()
                if token and token not in arg_tokens:
                    arg_tokens.append(token)

        # Handle arguments-desc
        raw_args_desc = meta.get("arguments-desc", "").strip()
        if raw_args_desc:
            import re
            clean_desc = re.sub(r'\n+', r'\\n', raw_args_desc.strip())
            args_desc_blocks.append(f"[{mod_name}]\\n{clean_desc}")

        # Handle desc
        raw_desc = meta.get("desc", "").strip()
        if raw_desc:
            import re
            clean_desc = re.sub(r'\n+', r'\\n', raw_desc.strip())
            desc_blocks.append(f"✦ {mod_name}: {clean_desc}")

    args_line = ",".join(arg_tokens) if arg_tokens else None
    
    def _beautify_and_truncate(blocks: List[str], max_len: int = 3800, prefix: str = "") -> Optional[str]:
        """Beautify and truncate description blocks to prevent Surge metadata overflow.
        
        Hard cap at ~3800 chars to stay safely under Surge's 4096 limit.
        """
        if not blocks:
            return None
        combined = prefix + "\\n\\n".join(blocks)
        if len(combined) > max_len:
            return combined[:max_len] + "…\\n(see upstream docs for full details)"
        return combined

    args_desc_line = _beautify_and_truncate(args_desc_blocks, 3800)
    desc_line = _beautify_and_truncate(desc_blocks, 3800, prefix="【Bundled Modules】\\n")
    return args_line, args_desc_line, desc_line


def merge_upstream_modules(
    sources: Sequence[SourceSpec],
    output_path: str,
    *,
    header_meta: Dict[str, str],
    provenance_comment: Optional[str] = None,
    optional_labels: Optional[Sequence[str]] = None,
    local_fallbacks: Optional[Dict[str, str]] = None,
    content_replacements: Optional[List[Tuple[str, str]]] = None,
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
            if content_replacements:
                for old_str, new_str in content_replacements:
                    text = text.replace(old_str, new_str)
        except RuntimeError as exc:
            if label in optional:
                Logger.warn(f"Skipping optional source {label}: {exc}")
                continue
            raise
        meta, sections = parse_module(text)
        parsed_parts.append((label, meta, sections))
        used_sources.append((label, url))
        for name, lines in sections:
            filtered_lines = [line for line in lines if line.strip()]
            if name in merged_sections and merged_sections[name] and filtered_lines:
                if merged_sections[name][-1] != "":
                    merged_sections[name].append("")
            merged_sections.setdefault(name, []).extend(filtered_lines)

    if not parsed_parts:
        raise RuntimeError(f"No upstream modules downloaded for {output_path}")

    combined_args, combined_args_desc, combined_desc = _combine_metadata(
        [(label, meta) for label, meta, _ in parsed_parts]
    )

    out_meta = dict(header_meta)
    out_meta["date"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    if combined_args:
        out_meta["arguments"] = combined_args
    if combined_args_desc:
        out_meta["arguments-desc"] = combined_args_desc
    if combined_desc:
        existing_desc = out_meta.get("desc", "").strip()
        if existing_desc:
            out_meta["desc"] = existing_desc + "\\n\\n" + combined_desc
        else:
            out_meta["desc"] = combined_desc

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
