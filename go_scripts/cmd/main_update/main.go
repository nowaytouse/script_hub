package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
	"github.com/nyamiiko/script_hub/go_scripts/pkg/pipeline"
	"github.com/nyamiiko/script_hub/go_scripts/pkg/qa"
	"github.com/nyamiiko/script_hub/go_scripts/pkg/tools"
)

func runQaScript(desc string, fn func() int) bool {
	fmt.Println(strings.Repeat("=", 60))
	fmt.Printf("🚀 %s\n", desc)
	fmt.Println(strings.Repeat("=", 60))
	if fn() != 0 {
		fmt.Printf("❌ %s failed.\n", desc)
		return false
	}
	fmt.Printf("✅ %s passed.\n", desc)
	return true
}

func main() {
	quick := flag.Bool("quick", false, "Skip heavy sync operations")
	execute := flag.Bool("execute", false, "Apply all changes and push")
	unattended := flag.Bool("unattended", false, "CI/local unattended mode (same as --execute)")
	force := flag.Bool("force", false, "Force update everything")
	withCore := flag.Bool("with-core", false, "Update local sing-box/mihomo binaries (optional, not for CI)")
	flag.Parse()

	if *unattended {
		*execute = true
	}

	startTime := time.Now()
	fmt.Println(strings.Repeat("=", 60))
	fmt.Println("🚀 Script Hub Go Update Tool")
	fmt.Println(strings.Repeat("=", 60))

	hasFailures := false

	if !runQaScript("Surge Compliance Tests", func() int {
		return qa.RunTestSurgeCompliance()
	}) {
		fmt.Println("Aborting pipeline due to compliance test failures.")
		os.Exit(1)
	}

	if !runQaScript("Module Header Validation", func() int {
		return qa.RunValidateModuleHeaders()
	}) {
		fmt.Println("Aborting pipeline due to module header validation failures.")
		os.Exit(1)
	}

	if *withCore {
		fmt.Println("\n--- Core Binary Update ---")
		tools.RunUpdateCores()
	}

	if !*quick {
		fmt.Println("\n--- Upstream Sync ---")
		syncer := pipeline.NewUpstreamSyncer()
		syncer.SyncSkk()
		syncer.SyncNexus()
		syncer.SyncMetacubex()
		syncer.SyncLocalSources()
		syncer.SyncBlockedSitesKorea()

		fmt.Println("\n--- Syncing Upstream Mock Resources ---")
		count := pipeline.SyncMocks()
		fmt.Printf("✅ Synced %d mock resources from upstream.\n", count)
	} else {
		fmt.Println("ℹ️ Quick mode: Skipping upstream sync.")
	}

	fmt.Println("\n--- Upstream Bundle Merges ---")
	bundleFailures := pipeline.MergeAllBundles(false)
	if len(bundleFailures) > 0 {
		fmt.Printf("⚠️ Upstream bundle merges incomplete (%s); continuing pipeline.\n", strings.Join(bundleFailures, ", "))
		hasFailures = true
	} else {
		fmt.Println("✅ Upstream bundle merges completed successfully.")
	}

	if !runQaScript("Bundle Completeness Audit", func() int {
		return qa.RunAuditBundleCompleteness()
	}) {
		hasFailures = true
	}

	fmt.Println("\n--- Smart-Config-Kit Supplemental Merge ---")
	pipeline.MergeSmartConfigKit()
	fmt.Println("✅ Smart-Config-Kit supplemental sources refreshed.")

	fmt.Println("\n--- Ruleset Manager ---")
	pipeline.RunRulesetManager(*force)

	fmt.Println("\n--- Ruleset Cleanup ---")
	cleanupStats := pipeline.RunCleanup()
	fmt.Printf("✅ Ruleset cleanup completed (deleted: %d)\n", cleanupStats["deleted"])

	fmt.Println("\n--- AdBlock Manager ---")
	adMgr := pipeline.NewAdBlockManager()
	adMgr.Merge(*execute || *force)

	fmt.Println("\n--- Final Ruleset Cleanup ---")
	cleanupStats2 := pipeline.RunCleanup()
	fmt.Printf("✅ Final ruleset cleanup completed (deleted: %d)\n", cleanupStats2["deleted"])

	if !runQaScript("Ruleset Compliance Validation", func() int {
		return qa.RunValidateSurgeRulesets()
	}) {
		hasFailures = true
	}

	fmt.Println("\n--- Firewall Sync ---")
	pipeline.SyncPorts(*execute || *force)

	fmt.Println("\n--- Module Processing & Conversion ---")
	if !runQaScript("Module Consolidation", func() int {
		return tools.RunConsolidateModules()
	}) {
		hasFailures = true
	}

	if !runQaScript("Shadowrocket Module Conversion", func() int {
		return tools.RunConvertSurgeToShadowrocket(map[string]string{"modules": "true"})
	}) {
		hasFailures = true
	}

	surgeConf := filepath.Join(hub.ROOT, ".claude/NyaMiiKo.conf.conf")
	if hub.ValidateFileExists(surgeConf, "") {
		tools.RunConvertSurgeToShadowrocket(map[string]string{"config": surgeConf})
	}

	fmt.Println("\n--- MITM Hardening ---")
	mitmCount := pipeline.RunMitmCleanup(filepath.Join(hub.ROOT, "modules/surge"), false)
	fmt.Printf("✅ MITM hardening completed: %d modules reinforced.\n", mitmCount)

	fmt.Println("\n--- Global Resource Localization & CDN Rewriting ---")
	pipeline.CopyGithubVariants()
	rwCount1 := pipeline.RunUrlRewrites(filepath.Join(hub.ROOT, "modules"))
	rwCount2 := pipeline.RunUrlRewrites(filepath.Join(hub.ROOT, "rulesets"))
	rwCount3 := pipeline.RunUrlRewrites(hub.ROOT)
	fmt.Printf("✅ URL rewrite completed: %d files redirected to CDN/local mocks.\n", rwCount1+rwCount2+rwCount3)

	fmt.Println("\n--- SRS Generator ---")
	srsGen := pipeline.NewSRSGenerator()
	srsGen.Run()

	if *execute {
		if hasFailures {
			fmt.Println("❌ Pipeline has errors. Skipping Git operations to prevent pushing broken state.")
		} else {
			fmt.Println("\n--- Git Operations ---")
			timestamp := time.Now().Format("2006-01-02 15:04")
			commitMsg := fmt.Sprintf("chore(ruleset): automated update %s CST (Go Version)", timestamp)

			cmd := exec.Command("git", "status", "--porcelain")
			cmd.Dir = hub.ROOT
			out, err := cmd.Output()
			if err != nil || len(strings.TrimSpace(string(out))) == 0 {
				fmt.Println("ℹ️ No changes to commit (working tree clean).")
			} else {
				cmd = exec.Command("git", "add", ".")
				cmd.Dir = hub.ROOT
				cmd.Run()

				cmd = exec.Command("git", "commit", "-m", commitMsg)
				cmd.Dir = hub.ROOT
				cmd.Run()

				if strings.ToLower(os.Getenv("PUSH_COOLDOWN_ENABLED")) == "true" {
					fmt.Println("ℹ️ Push cooldown: waiting 180s, then rebase onto origin/master and push...")
					time.Sleep(180 * time.Second)

					cmd = exec.Command("git", "fetch", "origin", "master")
					cmd.Dir = hub.ROOT
					cmd.Run()

					cmd = exec.Command("git", "rebase", "origin/master")
					cmd.Dir = hub.ROOT
					cmd.Run()
				}

				cmd = exec.Command("git", "push", "origin", "master")
				cmd.Dir = hub.ROOT
				if err := cmd.Run(); err != nil {
					fmt.Printf("⚠️ Git push failed: %v\n", err)
					hasFailures = true
				} else {
					fmt.Println("✅ Changes pushed to GitHub successfully.")
				}
			}
		}
	}

	duration := time.Since(startTime)
	if hasFailures {
		fmt.Printf("❌ Pipeline completed with errors. (Duration: %v)\n", duration)
		os.Exit(1)
	}
	fmt.Printf("🎉 All Tasks Completed Successfully! (Duration: %v)\n", duration)
}
