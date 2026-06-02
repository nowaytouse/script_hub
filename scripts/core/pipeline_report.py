#!/usr/bin/env python3
"""Post-run inventory: rulesets, AdBlock shards, modules, SRS coverage."""

from __future__ import annotations

import os
from typing import List, Tuple

from core.common import Logger, get_project_root, read_file

ROOT = get_project_root()
SURGE_DIR = os.path.join(ROOT, "rulesets/surge-shadowrocket")
DNS_MAPPING_DIR = os.path.join(ROOT, "rulesets/Sources/DNS_mapping")
DNS_RULES_DIR = os.path.join(ROOT, "rulesets/dns")
ADBLOCK_DIR = os.path.join(ROOT, "rulesets/AdBlock")
SINGBOX_DIR = os.path.join(ROOT, "rulesets/SingBox")
MODULE_SURGE = os.path.join(ROOT, "modules/surge")
MODULE_SR = os.path.join(ROOT, "modules/shadowrocket")

SRS_SOURCE_DIRS = [SURGE_DIR, DNS_MAPPING_DIR, ADBLOCK_DIR]
SRS_SKIP_PREFIXES = ("AdBlock_",)  # large shards: optional compile, still audited


def _rule_count(path: str) -> int:
    n = 0
    for line in read_file(path):
        s = line.strip()
        if s and not s.startswith("#"):
            n += 1
    return n


def _collect_list_files() -> List[str]:
    files: List[str] = []
    for directory in SRS_SOURCE_DIRS:
        if not os.path.isdir(directory):
            continue
        for name in sorted(os.listdir(directory)):
            if name.endswith(".list"):
                files.append(os.path.join(directory, name))
    return files


def audit_srs_coverage() -> Tuple[int, List[str]]:
    missing: List[str] = []
    total = 0
    for list_path in _collect_list_files():
        base = os.path.splitext(os.path.basename(list_path))[0]
        if any(base.startswith(p) for p in SRS_SKIP_PREFIXES):
            continue
        if _rule_count(list_path) == 0:
            continue
        total += 1
        srs_path = os.path.join(SINGBOX_DIR, f"{base}_Singbox.srs")
        # Some legitimately-small SRS outputs (e.g. ASN) still compile into a
        # valid binary with a stable header "SRS". Avoid false negatives by
        # validating the magic bytes instead of an arbitrary size threshold.
        if not os.path.isfile(srs_path):
            missing.append(f"{base}.list -> {os.path.basename(srs_path)}")
            continue
        try:
            with open(srs_path, "rb") as f:
                magic = f.read(3)
            if magic != b"SRS":
                missing.append(f"{base}.list -> {os.path.basename(srs_path)}")
        except Exception:
            missing.append(f"{base}.list -> {os.path.basename(srs_path)}")
    return total, missing


def print_pipeline_summary() -> None:
    Logger.section("Pipeline Summary")

    surge_lists = [
        f for f in os.listdir(SURGE_DIR) if f.endswith(".list")
    ] if os.path.isdir(SURGE_DIR) else []
    adblock_lists = [
        f for f in os.listdir(ADBLOCK_DIR) if f.endswith(".list")
    ] if os.path.isdir(ADBLOCK_DIR) else []
    srs_files = [
        f for f in os.listdir(SINGBOX_DIR) if f.endswith(".srs")
    ] if os.path.isdir(SINGBOX_DIR) else []

    surge_modules = 0
    for root, _dirs, files in os.walk(MODULE_SURGE):
        surge_modules += sum(1 for f in files if f.endswith(".sgmodule"))

    sr_modules = 0
    if os.path.isdir(MODULE_SR):
        for root, _dirs, files in os.walk(MODULE_SR):
            sr_modules += sum(1 for f in files if f.endswith(".module"))

    Logger.info(f"Surge rulesets: {len(surge_lists)} | AdBlock shards: {len(adblock_lists)}")
    Logger.info(f"Sing-box SRS: {len(srs_files)}")
    Logger.info(f"Surge modules: {surge_modules} | Shadowrocket modules: {sr_modules}")

    checked, missing = audit_srs_coverage()
    if missing:
        Logger.warn(f"SRS coverage: {len(missing)}/{checked} list files missing SRS")
        for item in missing[:15]:
            Logger.warn(f"  Missing: {item}")
        if len(missing) > 15:
            Logger.warn(f"  ... and {len(missing) - 15} more")
    else:
        Logger.success(f"SRS coverage OK ({checked} compiled sources checked)")
