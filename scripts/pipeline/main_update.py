#!/usr/bin/env python3
import os
import argparse
import subprocess
import sys
import time
from datetime import datetime
from zoneinfo import ZoneInfo

from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hub.common import Logger, is_ci
from hub.pipeline_report import print_pipeline_summary
from hub.project_paths import ROOT, SCRIPTS_DIR

CORE_UPDATE_SCRIPT = os.path.join(SCRIPTS_DIR, "qa/update_cores.sh")

from pipeline.firewall_sync import sync_ports
from pipeline.upstream_sync import UpstreamSyncer
from pipeline.adblock_manager import AdBlockManager
from pipeline.merge_smart_config_kit import merge as merge_smart_config_kit
from pipeline.ruleset_manager import RulesetManager
from pipeline.srs_generator import SRSGenerator
from pipeline.mitm_cleanup_github import run_cleanup
from pipeline.smart_cleanup import (
    run_cleanup as run_ruleset_cleanup,
    format_stats as format_ruleset_cleanup_stats,
)
from pipeline.merge_bundles import run_all as run_upstream_bundles

CI_PUSH_COOLDOWN_MIN_SECONDS = 180


def _push_cooldown_seconds() -> int:
    raw = os.environ.get("PUSH_COOLDOWN_SECONDS", "").strip()
    if raw.isdigit():
        seconds = int(raw)
    elif is_ci():
        seconds = CI_PUSH_COOLDOWN_MIN_SECONDS
    else:
        seconds = 1800
    if is_ci():
        seconds = max(seconds, CI_PUSH_COOLDOWN_MIN_SECONDS)
    return seconds


def _git_sync_before_push() -> None:
    subprocess.run(["git", "fetch", "origin", "master"], check=True, cwd=ROOT)
    subprocess.run(["git", "rebase", "origin/master"], check=True, cwd=ROOT)


def _py_env() -> dict:
    return {**os.environ, "PYTHONPATH": SCRIPTS_DIR}


def main():
    parser = argparse.ArgumentParser(description="Script Hub Python Main Orchestrator (v1.4)")
    parser.add_argument("--quick", action="store_true", help="Skip heavy sync operations")
    parser.add_argument("--execute", action="store_true", help="Apply all changes and push")
    parser.add_argument(
        "--unattended",
        action="store_true",
        help="CI/local unattended mode (same as --execute)",
    )
    parser.add_argument("--force", action="store_true", help="Force update everything")
    parser.add_argument(
        "--with-core",
        action="store_true",
        help="Update local sing-box/mihomo binaries (optional, not for CI)",
    )
    args = parser.parse_args()
    if args.unattended:
        args.execute = True

    start_time = datetime.now()
    Logger.section("Script Hub Python Update Tool (v1.4)")
    has_failures = False

    Logger.section("Surge Compliance Tests")
    try:
        result = subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "qa/test_surge_compliance.py")],
            cwd=ROOT,
            env=_py_env(),
        )
        if result.returncode != 0:
            Logger.error("Surge compliance regression tests failed.")
            has_failures = True
        else:
            Logger.success("Surge compliance regression tests passed.")
    except Exception as e:
        Logger.error(f"Compliance tests failed: {e}")
        has_failures = True

    if has_failures:
        Logger.error("Aborting pipeline due to compliance test failures.")
        sys.exit(1)

    Logger.section("Module Header Validation")
    try:
        result = subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "qa/validate_module_headers.py")],
            cwd=ROOT,
            env=_py_env(),
        )
        if result.returncode != 0:
            Logger.error("Module header validation failed.")
            has_failures = True
        else:
            Logger.success("Module header validation passed.")
    except Exception as e:
        Logger.error(f"Module header validation failed: {e}")
        has_failures = True

    if has_failures:
        Logger.error("Aborting pipeline due to module header validation failures.")
        sys.exit(1)

    if args.with_core:
        Logger.section("Core Binary Update")
        if os.path.isfile(CORE_UPDATE_SCRIPT):
            result = subprocess.run(["bash", CORE_UPDATE_SCRIPT], cwd=ROOT)
            if result.returncode != 0:
                Logger.warn("Core update script exited with errors (continuing pipeline).")
        else:
            Logger.warn(f"Core updater not found: {CORE_UPDATE_SCRIPT}")

    if not args.quick:
        try:
            syncer = UpstreamSyncer()
            syncer.sync_skk()
            syncer.sync_nexus()
            syncer.sync_metacubex()
            syncer.sync_local_sources()
        except Exception as e:
            Logger.error(f"Upstream sync failed: {e}")
            has_failures = True
    else:
        Logger.info("Quick mode: Skipping upstream sync.")

    Logger.section("Upstream Bundle Merges")
    bundle_failures = run_upstream_bundles()
    if bundle_failures:
        Logger.warn(
            f"Upstream bundle merges incomplete ({', '.join(bundle_failures)}); continuing pipeline."
        )
        has_failures = True
    else:
        Logger.success("Upstream bundle merges completed successfully.")

    Logger.section("Bundle Completeness Audit")
    try:
        audit_result = subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "qa/audit_bundle_completeness.py")],
            cwd=ROOT,
            env=_py_env(),
        )
        if audit_result.returncode != 0:
            Logger.error("Bundle completeness audit failed.")
            has_failures = True
        else:
            Logger.success("Bundle completeness audit passed.")
    except Exception as e:
        Logger.error(f"Bundle completeness audit failed: {e}")
        has_failures = True

    Logger.section("Smart-Config-Kit Supplemental Merge")
    try:
        merge_smart_config_kit()
        Logger.success("Smart-Config-Kit supplemental sources refreshed.")
    except Exception as e:
        Logger.error(f"Smart-Config-Kit merge failed: {e}")
        has_failures = True

    try:
        rules_mgr = RulesetManager(force=args.force)
        rules_mgr.run()
    except Exception as e:
        Logger.error(f"Ruleset manager failed: {e}")
        has_failures = True

    Logger.section("Ruleset Cleanup")
    try:
        cleanup_stats = run_ruleset_cleanup()
        Logger.success(f"Ruleset cleanup completed: {format_ruleset_cleanup_stats(cleanup_stats)}")
    except Exception as e:
        Logger.error(f"Ruleset cleanup failed: {e}")
        has_failures = True

    try:
        ad_mgr = AdBlockManager()
        ad_mgr.merge(execute=args.execute or args.force)
    except Exception as e:
        Logger.error(f"AdBlock manager failed: {e}")
        has_failures = True

    Logger.section("Repair script-path URLs")
    try:
        result = subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "tools/repair_script_paths.py")],
            cwd=ROOT,
            env=_py_env(),
        )
        if result.returncode != 0:
            Logger.warn("script-path repair exited non-zero (continuing).")
    except Exception as e:
        Logger.warn(f"script-path repair failed: {e}")

    Logger.section("Final Ruleset Cleanup")
    try:
        cleanup_stats = run_ruleset_cleanup()
        Logger.success(f"Final ruleset cleanup completed: {format_ruleset_cleanup_stats(cleanup_stats)}")
    except Exception as e:
        Logger.error(f"Final ruleset cleanup failed: {e}")
        has_failures = True

    Logger.section("Ruleset Compliance Validation")
    try:
        result = subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "qa/validate_surge_rulesets.py")],
            cwd=ROOT,
            env=_py_env(),
        )
        if result.returncode != 0:
            Logger.error("Ruleset compliance validation failed.")
            has_failures = True
        else:
            Logger.success("Ruleset compliance validation passed.")
    except Exception as e:
        Logger.error(f"Ruleset validation failed: {e}")
        has_failures = True

    try:
        sync_ports(execute=args.execute or args.force)
    except Exception as e:
        Logger.error(f"Firewall port sync failed: {e}")
        has_failures = True

    Logger.section("Module Processing & Conversion")
    try:
        subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "tools/consolidate_modules.py")],
            check=True,
            cwd=ROOT,
            env=_py_env(),
        )
        conv = subprocess.run(
            [
                sys.executable,
                os.path.join(SCRIPTS_DIR, "tools/convert_surge_to_shadowrocket.py"),
                "--modules",
            ],
            cwd=ROOT,
            env=_py_env(),
        )
        if conv.returncode != 0:
            Logger.error("Shadowrocket module conversion failed (no modules converted).")
            has_failures = True
        surge_conf = os.path.join(ROOT, ".claude", "NyaMiiKo.conf.conf")
        if os.path.isfile(surge_conf):
            subprocess.run(
                [
                    sys.executable,
                    os.path.join(SCRIPTS_DIR, "tools/convert_surge_to_shadowrocket.py"),
                    "--config",
                    surge_conf,
                ],
                check=False,
                cwd=ROOT,
                env=_py_env(),
            )
        Logger.success("Module processing and conversion completed.")
    except Exception as e:
        Logger.error(f"Module processing failed: {e}")
        has_failures = True

    Logger.section("MITM Hardening")
    try:
        count = run_cleanup()
        Logger.success(f"MITM hardening completed: {count} modules reinforced.")
    except Exception as e:
        Logger.error(f"MITM hardening failed: {e}")
        has_failures = True

    try:
        srs_gen = SRSGenerator()
        srs_gen.run()
    except Exception as e:
        Logger.error(f"SRS generator failed: {e}")
        has_failures = True

    if args.execute:
        if has_failures:
            Logger.error("Pipeline has errors. Skipping Git operations to prevent pushing broken state.")
        else:
            Logger.section("Git Operations")
            timestamp = datetime.now(ZoneInfo("Asia/Shanghai")).strftime("%Y-%m-%d %H:%M")
            commit_msg = f"chore(ruleset): automated update {timestamp} CST"
            try:
                status = subprocess.run(
                    ["git", "status", "--porcelain"],
                    capture_output=True,
                    text=True,
                    cwd=ROOT,
                )
                if not (status.stdout or "").strip():
                    Logger.info("No changes to commit (working tree clean).")
                else:
                    subprocess.run(["git", "add", "."], check=True, cwd=ROOT)
                    subprocess.run(
                        ["git", "commit", "-m", commit_msg],
                        check=True,
                        cwd=ROOT,
                    )
                    if os.environ.get("PUSH_COOLDOWN_ENABLED", "").lower() == "true":
                        secs = _push_cooldown_seconds()
                        Logger.info(
                            f"Push cooldown: waiting {secs}s, then rebase onto origin/master and push..."
                        )
                        time.sleep(secs)
                        _git_sync_before_push()
                    subprocess.run(["git", "push", "origin", "master"], check=True, cwd=ROOT)
                    Logger.success("Changes pushed to GitHub successfully.")
            except Exception as e:
                Logger.warn(f"Git operation failed: {e}")
                has_failures = True

    print_pipeline_summary()
    duration = datetime.now() - start_time
    if has_failures:
        Logger.error(f"Pipeline completed with errors. (Duration: {duration})")
        sys.exit(1)
    Logger.section(f"All Tasks Completed Successfully! (Duration: {duration})")


if __name__ == "__main__":
    main()
