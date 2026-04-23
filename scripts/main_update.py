#!/usr/bin/env python3
import os
import argparse
import sys
from datetime import datetime
from lib.common import Logger, get_project_root

# Import our managers
from firewall_sync import sync_ports
from upstream_sync import UpstreamSyncer
from adblock_manager import AdBlockManager
from ruleset_manager import RulesetManager
from srs_generator import SRSGenerator
from maintenance.mitm_cleanup_github import run_cleanup
from smart_cleanup import run_cleanup as run_ruleset_cleanup, format_stats as format_ruleset_cleanup_stats

# CONFIGURATION

ROOT = get_project_root()
SCRIPTS_DIR = os.path.join(ROOT, "scripts")

# MAIN ORCHESTRATOR

def main():
    parser = argparse.ArgumentParser(description="Script Hub Python Main Orchestrator (v1.2 International)")
    parser.add_argument("--quick", action="store_true", help="Skip heavy sync operations")
    parser.add_argument("--execute", action="store_true", help="Apply all changes and push")
    parser.add_argument("--force", action="store_true", help="Force update everything")
    args = parser.parse_args()

    start_time = datetime.now()
    Logger.section("Script Hub Python Update Tool (v1.2 Native English)")

    # 1. Upstream Sync
    if not args.quick:
        syncer = UpstreamSyncer()
        syncer.sync_skk()
        syncer.sync_nexus()
        syncer.sync_metacubex()
    else:
        Logger.info("Quick mode: Skipping upstream sync.")

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

    # 3.1 Final ruleset cleanup after AdBlock rebuild to keep payloads unique
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
        import subprocess
        # Run conversion script
        subprocess.run(["python3", os.path.join(SCRIPTS_DIR, "convert_surge_to_shadowrocket.py")], check=True)
        # Run consolidation script
        subprocess.run(["python3", os.path.join(SCRIPTS_DIR, "consolidate_modules.py")], check=True)
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
        import subprocess
        timestamp = datetime.now().strftime('%Y-%m-%d %H:%M')
        commit_msg = f"chore(ruleset): international-python-update {timestamp}"
        try:
            subprocess.run(["git", "add", "."], check=True)
            status = subprocess.run(["git", "status", "--porcelain"], capture_output=True, text=True)
            if status.stdout:
                subprocess.run(["git", "commit", "-m", commit_msg], check=True)
                subprocess.run(["git", "push", "origin", "master"], check=True)
                Logger.success("Changes pushed to GitHub successfully.")
            else:
                Logger.info("No changes to commit (working tree clean).")
        except Exception as e:
            Logger.warn(f"Git operation failed: {e}")

    duration = datetime.now() - start_time
    Logger.section(f"All Tasks Completed Successfully! (Duration: {duration})")

if __name__ == "__main__":
    main()
