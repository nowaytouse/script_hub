package qa

import (
	"fmt"
	"strings"

	"github.com/nyamiiko/script_hub/create/hub"
)

func assertEq(label string, got, want string) error {
	if got != want {
		return fmt.Errorf("%s: got %q, want %q", label, got, want)
	}
	return nil
}

func assertNil(label string, got *string) error {
	if got != nil {
		return fmt.Errorf("%s: expected no error/nil, got %q", label, *got)
	}
	return nil
}

func assertErr(label string, err error, substr string) error {
	if err == nil || !strings.Contains(err.Error(), substr) {
		return fmt.Errorf("%s: expected error containing %q, got %v", label, substr, err)
	}
	return nil
}

func testUrlRegexNotTruncated() error {
	raw := `URL-REGEX,https://www\.google\.com/.*continue=https://gemini\.google\.com.+`
	if err := assertEq("strip comment", hub.StripInlineComment(raw), raw); err != nil {
		return err
	}
	p := hub.NewRuleProcessor(false)
	out := p.NormalizeRule(raw, "")
	if out == nil {
		return fmt.Errorf("URL-REGEX must not be dropped")
	}
	parts := strings.SplitN(*out, ",", 2)
	payload := parts[1]
	if !strings.Contains(payload, "gemini") {
		return fmt.Errorf("truncated payload: %q", payload)
	}
	if payload == "https:" {
		return fmt.Errorf("truncated to scheme only: %q", payload)
	}
	return nil
}

func testInvalidUrlRegexRejected() error {
	p := hub.NewRuleProcessor(false)
	if p.NormalizeRule("URL-REGEX,https:", "") != nil {
		return fmt.Errorf("URL-REGEX,https: not rejected")
	}
	if p.NormalizeRule("URL-REGEX,^https?://", "") != nil {
		return fmt.Errorf("URL-REGEX,^https?:// not rejected")
	}
	return nil
}

func testProcessNamePreserved() error {
	p := hub.NewRuleProcessor(false)
	if err := assertEq("PROCESS-NAME", *p.NormalizeRule("PROCESS-NAME,Music", ""), "PROCESS-NAME,Music"); err != nil {
		return err
	}
	if err := assertEq("USER-AGENT", *p.NormalizeRule("USER-AGENT,*Music?", ""), "USER-AGENT,*Music?"); err != nil {
		return err
	}
	return nil
}

func testIpCidrNoResolve() error {
	p := hub.NewRuleProcessor(false)
	if err := assertEq("IP-CIDR no-resolve", *p.NormalizeRule("IP-CIDR,23.41.4.0/22,no-resolve", ""), "IP-CIDR,23.41.4.0/22"); err != nil {
		return err
	}
	return nil
}

func testDomainRegexJunkDropped() error {
	p := hub.NewRuleProcessor(false)
	if p.NormalizeRule("DOMAIN-REGEX,$", "") != nil {
		return fmt.Errorf("DOMAIN-REGEX,$ not rejected")
	}
	if p.NormalizeRule("DOMAIN-REGEX,c", "") != nil {
		return fmt.Errorf("DOMAIN-REGEX,c not rejected")
	}
	return nil
}

func testNetflixDomainRegexConverted() error {
	p := hub.NewRuleProcessor(false)
	raw := `DOMAIN-REGEX,(^|\.)apiproxy-device-prod-nlb-.+\.amazonaws\.com$`
	normalized := p.NormalizeRule(raw, "")
	if normalized == nil {
		return fmt.Errorf("normalized is nil")
	}
	converted := hub.ConvertDomainRegexForSurge(*normalized)
	if err := assertEq("netflix convert", converted, "DOMAIN-KEYWORD,apiproxy-device-prod-nlb"); err != nil {
		return err
	}
	return nil
}

func testSurgeListForbidsDomainRegex() error {
	line := `DOMAIN-REGEX,"(^|\.)foo$"`
	errStr := hub.ValidateSurgeRulesetLine(line, false)
	if errStr == nil || !strings.Contains(*errStr, "not supported") {
		return fmt.Errorf("forbidden: expected error containing %q, got %v", "not supported", errStr)
	}
	return nil
}

func testSurgeListAllowsKeyword() error {
	errStr := hub.ValidateSurgeRulesetLine("DOMAIN-KEYWORD,apiproxy-device-prod-nlb", false)
	if errStr != nil {
		return fmt.Errorf("keyword ok: expected no error, got %v", *errStr)
	}
	return nil
}

func RunTestSurgeCompliance() int {
	tests := []struct {
		name string
		fn   func() error
	}{
		{"testUrlRegexNotTruncated", testUrlRegexNotTruncated},
		{"testInvalidUrlRegexRejected", testInvalidUrlRegexRejected},
		{"testProcessNamePreserved", testProcessNamePreserved},
		{"testIpCidrNoResolve", testIpCidrNoResolve},
		{"testDomainRegexJunkDropped", testDomainRegexJunkDropped},
		{"testNetflixDomainRegexConverted", testNetflixDomainRegexConverted},
		{"testSurgeListForbidsDomainRegex", testSurgeListForbidsDomainRegex},
		{"testSurgeListAllowsKeyword", testSurgeListAllowsKeyword},
	}
	failed := 0
	for _, t := range tests {
		if err := t.fn(); err != nil {
			fmt.Printf("  FAIL %s: %v\n", t.name, err)
			failed++
		} else {
			fmt.Printf("  OK  %s\n", t.name)
		}
	}
	if failed > 0 {
		fmt.Printf("\n%d/%d failed\n", failed, len(tests))
		return 1
	}
	fmt.Printf("\nAll %d compliance tests passed\n", len(tests))
	return 0
}
