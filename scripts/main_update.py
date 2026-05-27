#!/usr/bin/env python3
import os
import argparse
import subprocess
import sys
from datetime import datetime
from lib.common import Logger, get_project_root
from lib.pipeline_report import print_pipeline_summary

# Import our managers
from firewall_sync import sync_ports
from upstream_sync import UpstreamSyncer
from adblock_manager import AdBlockManager
from merge_smart_config_kit import merge as merge_smart_config_kit
from ruleset_manager import RulesetManager
from srs_generator import SRSGenerator
from maintenance.mitm_cleanup_github import run_cleanup
from smart_cleanup import run_cleanup as run_ruleset_cleanup, format_stats as format_ruleset_cleanup_stats

# CONFIGURATION

ROOT = get_project_root()
SCRIPTS_DIR = os.path.join(ROOT, "scripts")
CORE_UPDATE_SCRIPT = os.path.join(SCRIPTS_DIR, "tools/update_cores.sh")

# MAIN ORCHESTRATOR

def main():
    parser = argparse.ArgumentParser(description="Script Hub Python Main Orchestrator (v1.3)")
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
    Logger.section("Script Hub Python Update Tool (v1.3)")

    if args.with_core:
        Logger.section("Core Binary Update")
        if os.path.isfile(CORE_UPDATE_SCRIPT):
            result = subprocess.run(["bash", CORE_UPDATE_SCRIPT], cwd=ROOT)
            if result.returncode != 0:
                Logger.warn("Core update script exited with errors (continuing pipeline).")
        else:
            Logger.warn(f"Core updater not found: {CORE_UPDATE_SCRIPT}")

    # 1. Upstream Sync
    if not args.quick:
        syncer = UpstreamSyncer()
        syncer.sync_skk()
        syncer.sync_nexus()
        syncer.sync_metacubex()
    else:
        Logger.info("Quick mode: Skipping upstream sync.")

    # 1.1 Smart-Config-Kit supplemental rules merge
    # Source files vendored at ruleset/Sources/custom/SmartConfigKit/ (fork removed)
    Logger.section("Smart-Config-Kit Supplemental Merge")
    try:
        merge_smart_config_kit()
        Logger.success("Smart-Config-Kit supplemental sources refreshed.")
    except Exception as e:
        Logger.error(f"Smart-Config-Kit merge failed: {e}")

    # 2. Ruleset Merge (Incremental merge + header injection)
    rules_mgr = RulesetManager(force=args.force)
    rules_mgr.run()

    # 2.1 Cross-ruleset cleanup (rebalance generic Direct/Proxy + payload dedupe)
    Logger.section("Ruleset Cleanup")
    try:
        cleanup_stats = run_ruleset_cleanup()
        Logger.success(f"Ruleset cleanup completed: {format_ruleset_cleanup_stats(cleanup_stats)}")
    except Exception as e:
        Logger.error(f"Ruleset cleanup failed: {e}")

    # 3. AdBlock Module & Ruleset Merge
    ad_mgr = AdBlockManager()
    ad_mgr.merge(execute=args.execute or args.force)

    # 3.2 Final ruleset cleanup after AdBlock rebuild to keep payloads unique
    Logger.section("Final Ruleset Cleanup")
    try:
        cleanup_stats = run_ruleset_cleanup()
        Logger.success(f"Final ruleset cleanup completed: {format_ruleset_cleanup_stats(cleanup_stats)}")
    except Exception as e:
        Logger.error(f"Final ruleset cleanup failed: {e}")

    # 4. Firewall Port Sync
    sync_ports(execute=args.execute or args.force)

    # 5. Module Consolidation & Shadowrocket Conversion
    Logger.section("Module Processing & Conversion")
    try:
        subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "convert_surge_to_shadowrocket.py")],
            check=True,
        )
        subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "consolidate_modules.py")],
            check=True,
        )
        Logger.success("Module processing and conversion completed.")
    except Exception as e:
        Logger.error(f"Module processing failed: {e}")

    # 6. MITM Hardening (Final cleanup pass)
    Logger.section("MITM Hardening")
    try:
        count = run_cleanup()
        Logger.success(f"MITM hardening completed: {count} modules reinforced.")
    except Exception as e:
        Logger.error(f"MITM hardening failed: {e}")

    # 7. SRS Generation
    srs_gen = SRSGenerator()
    srs_gen.run()

    # 7. Git Commit & Push
    if args.execute:
        Logger.section("Git Operations")
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M")
        commit_msg = f"chore(ruleset): automated update {timestamp}"
        try:
            subprocess.run(["git", "add", "."], check=True, cwd=ROOT)
            status = subprocess.run(
                ["git", "status", "--porcelain"],
                capture_output=True,
                text=True,
                cwd=ROOT,
            )
            if status.stdout.strip():
                subprocess.run(
                    ["git", "commit", "-m", commit_msg],
                    check=True,
                    cwd=ROOT,
                )
                subprocess.run(["git", "push", "origin", "master"], check=True, cwd=ROOT)
                Logger.success("Changes pushed to GitHub successfully.")
            else:
                Logger.info("No changes to commit (working tree clean).")
        except Exception as e:
            Logger.warn(f"Git operation failed: {e}")

    print_pipeline_summary()
    duration = datetime.now() - start_time
    Logger.section(f"All Tasks Completed Successfully! (Duration: {duration})")

if __name__ == "__main__":
    main()
