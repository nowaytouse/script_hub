package pipeline

import (
	"net"
	"testing"
)

func TestDetermineCategory(t *testing.T) {
	m := NewAdBlockManager()
	tests := []struct {
		source   string
		expected string
	}{
		{"https://raw.githubusercontent.com/privacy-protection-tools/anti-AD/master/anti-ad-surge2.txt", "AntiAD"},
		{"https://raw.githubusercontent.com/hagezi/dns-blocklists/main/surge/ultimate.list", "ThreatIntel_Ultimate"},
		{"https://raw.githubusercontent.com/hagezi/dns-blocklists/main/surge/tif.medium.list", "ThreatIntel_TIF"},
		{"https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Advertising/Advertising.list", "Advertising"},
		{"https://raw.githubusercontent.com/some/unknown/list.list", "Other"},
	}

	for _, tt := range tests {
		cat := m.DetermineCategory(tt.source)
		if cat != tt.expected {
			t.Errorf("DetermineCategory(%q) = %q; want %q", tt.source, cat, tt.expected)
		}
	}
}

func TestCategoryFromFilename(t *testing.T) {
	m := NewAdBlockManager()
	tests := []struct {
		filename string
		expected string
	}{
		{"AdBlock_Local.list", "Local"},
		{"AdBlock_ThreatIntel_Ultimate_01.list", "ThreatIntel_Ultimate"},
		{"AdBlock_Advertising.list", "Advertising"},
		{"Random.list", "Other"},
	}

	for _, tt := range tests {
		cat := m.CategoryFromFilename(tt.filename)
		if cat != tt.expected {
			t.Errorf("CategoryFromFilename(%q) = %q; want %q", tt.filename, cat, tt.expected)
		}
	}
}

func TestIsWhitelisted(t *testing.T) {
	m := NewAdBlockManager()
	m.WhitelistDomain["google.com"] = true
	m.WhitelistSuffix["apple.com"] = true
	m.WhitelistKeyword["github"] = true

	_, network, _ := net.ParseCIDR("192.168.0.0/16")
	m.WhitelistIpNetworks = append(m.WhitelistIpNetworks, network)

	tests := []struct {
		payload  string
		expected bool
	}{
		{"google.com", true},
		{"sub.google.com", false}, // Exact match only for domain
		{"apple.com", true},
		{"sub.apple.com", true},
		{"githubusercontent.com", true},
		{"192.168.1.10", true},
		{"10.0.0.1", false},
	}

	for _, tt := range tests {
		result := m.IsWhitelisted(tt.payload)
		if result != tt.expected {
			t.Errorf("IsWhitelisted(%q) = %v; want %v", tt.payload, result, tt.expected)
		}
	}
}
