#!/usr/bin/env python3
import os
import argparse
import subprocess
import sys
import time
from datetime import datetime
from lib.common import Logger, get_project_root, is_ci
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

# Import maintenance bundle merges
from maintenance.merge_bilibili_bundle import main as merge_bilibili_bundle
from maintenance.merge_youtube_bundle import main as merge_youtube_bundle
from maintenance.merge_weibo_bundle import main as merge_weibo_bundle
from maintenance.merge_apple_modules import main as merge_apple_modules

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
    has_failures = False

    Logger.section("Surge Compliance Tests")
    try:
        result = subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "tools/test_surge_compliance.py")],
            cwd=ROOT,
            env={**os.environ, "PYTHONPATH": SCRIPTS_DIR},
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
        try:
            syncer = UpstreamSyncer()
            syncer.sync_skk()
            syncer.sync_nexus()
            syncer.sync_metacubex()
            syncer.sync_local_sources()   # Sync upstream modules → PROMAX local_sources
        except Exception as e:
            Logger.error(f"Upstream sync failed: {e}")
            has_failures = True
    else:
        Logger.info("Quick mode: Skipping upstream sync.")

    # 1.2 Upstream Bundle Merges (BiliBili, YouTube, Weibo, Apple, DNS)
    Logger.section("Upstream Bundle Merges")
    try:
        Logger.info("Executing BiliBili bundle merge...")
        merge_bilibili_bundle()
        Logger.info("Executing YouTube bundle merge (with ADBlock extraction)...")
        merge_youtube_bundle()
        Logger.info("Executing Weibo bundle merge...")
        merge_weibo_bundle()
        Logger.info("Executing Apple bundle merge...")
        merge_apple_modules()
        Logger.success("Upstream bundle merges completed successfully.")
    except Exception as e:
        Logger.error(f"Upstream bundle merges failed: {e}")
        has_failures = True

    # 1.1 Smart-Config-Kit supplemental rules merge
    # Source files vendored at ruleset/Sources/custom/SmartConfigKit/ (fork removed)
    Logger.section("Smart-Config-Kit Supplemental Merge")
    try:
        merge_smart_config_kit()
        Logger.success("Smart-Config-Kit supplemental sources refreshed.")
    except Exception as e:
        Logger.error(f"Smart-Config-Kit merge failed: {e}")
        has_failures = True

    # 2. Ruleset Merge (Incremental merge + header injection)
    try:
        rules_mgr = RulesetManager(force=args.force)
        rules_mgr.run()
    except Exception as e:
        Logger.error(f"Ruleset manager failed: {e}")
        has_failures = True

    # 2.1 Cross-ruleset cleanup (rebalance generic Direct/Proxy + payload dedupe)
    Logger.section("Ruleset Cleanup")
    try:
        cleanup_stats = run_ruleset_cleanup()
        Logger.success(f"Ruleset cleanup completed: {format_ruleset_cleanup_stats(cleanup_stats)}")
    except Exception as e:
        Logger.error(f"Ruleset cleanup failed: {e}")
        has_failures = True

    # 3. AdBlock Module & Ruleset Merge
    try:
        ad_mgr = AdBlockManager()
        ad_mgr.merge(execute=args.execute or args.force)
    except Exception as e:
        Logger.error(f"AdBlock manager failed: {e}")
        has_failures = True

    # 3.2 Final ruleset cleanup after AdBlock rebuild to keep payloads unique
    Logger.section("Final Ruleset Cleanup")
    try:
        cleanup_stats = run_ruleset_cleanup()
        Logger.success(f"Final ruleset cleanup completed: {format_ruleset_cleanup_stats(cleanup_stats)}")
    except Exception as e:
        Logger.error(f"Final ruleset cleanup failed: {e}")
        has_failures = True

    # 3.5 Compliance check (URL-REGEX / DOMAIN-REGEX / IP-CIDR)
    Logger.section("Ruleset Compliance Validation")
    try:
        result = subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "tools/validate_surge_rulesets.py")],
            cwd=ROOT,
            env={**os.environ, "PYTHONPATH": SCRIPTS_DIR},
        )
        if result.returncode != 0:
            Logger.error("Ruleset compliance validation failed.")
            has_failures = True
        else:
            Logger.success("Ruleset compliance validation passed.")
    except Exception as e:
        Logger.error(f"Ruleset validation failed: {e}")
        has_failures = True

    # 4. Firewall Port Sync
    try:
        sync_ports(execute=args.execute or args.force)
    except Exception as e:
        Logger.error(f"Firewall port sync failed: {e}")
        has_failures = True

    # 5. Module Consolidation & Shadowrocket Conversion
    # Order matters: consolidate first (generates / normalises Surge modules),
    # then convert so Shadowrocket gets the finalised output.
    Logger.section("Module Processing & Conversion")
    try:
        subprocess.run(
            [sys.executable, os.path.join(SCRIPTS_DIR, "consolidate_modules.py")],
            check=True,
            cwd=ROOT,
        )
        # Always refresh SR modules; main Surge conf lives under .claude (gitignored, local-only)
        conv = subprocess.run(
            [
                sys.executable,
                os.path.join(SCRIPTS_DIR, "convert_surge_to_shadowrocket.py"),
                "--modules",
            ],
            cwd=ROOT,
        )
        if conv.returncode != 0:
            Logger.error("Shadowrocket module conversion failed (no modules converted).")
            has_failures = True
        surge_conf = os.path.join(ROOT, ".claude", "NyaMiiKo.conf.conf")
        if os.path.isfile(surge_conf):
            subprocess.run(
                [
                    sys.executable,
                    os.path.join(SCRIPTS_DIR, "convert_surge_to_shadowrocket.py"),
                    "--config",
                    surge_conf,
                ],
                check=False,
                cwd=ROOT,
            )
        Logger.success("Module processing and conversion completed.")
    except Exception as e:
        Logger.error(f"Module processing failed: {e}")
        has_failures = True

    # 6. MITM Hardening (Final cleanup pass)
    Logger.section("MITM Hardening")
    try:
        count = run_cleanup()
        Logger.success(f"MITM hardening completed: {count} modules reinforced.")
    except Exception as e:
        Logger.error(f"MITM hardening failed: {e}")
        has_failures = True

    # 7. SRS Generation
    try:
        srs_gen = SRSGenerator()
        srs_gen.run()
    except Exception as e:
        Logger.error(f"SRS generator failed: {e}")
        has_failures = True

    # 7. Git Commit & Push
    if args.execute:
        if has_failures:
            Logger.error("Pipeline has errors. Skipping Git operations to prevent pushing broken state.")
        else:
            Logger.section("Git Operations")
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M")
            commit_msg = f"chore(ruleset): automated update {timestamp}"
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
                    if os.environ.get("PUSH_COOLDOWN_ENABLED", "").lower() == "true":
                        Logger.info(
                            "Push cooldown enabled: waiting 30 minutes before commit/push..."
                        )
                        time.sleep(1800)
                    subprocess.run(["git", "add", "."], check=True, cwd=ROOT)
                    subprocess.run(
                        ["git", "commit", "-m", commit_msg],
                        check=True,
                        cwd=ROOT,
                    )
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
    else:
        Logger.section(f"All Tasks Completed Successfully! (Duration: {duration})")

if __name__ == "__main__":
    main()
