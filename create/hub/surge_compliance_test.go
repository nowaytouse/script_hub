package hub

import "testing"

func TestValidateSurgeRulesetURLRegexOptionsAndCommas(t *testing.T) {
	tests := []struct {
		name string
		line string
		ok   bool
	}{
		{
			name: "extended matching option",
			line: `URL-REGEX,^https?:\/\/ads\.example\/,EXTENDED-MATCHING`,
			ok:   true,
		},
		{
			name: "quoted comma plus option",
			line: `URL-REGEX,"^https?:\/\/ads\.example\/(a{1,3}|b)",EXTENDED-MATCHING`,
			ok:   true,
		},
		{
			name: "unquoted payload comma",
			line: `URL-REGEX,^https?:\/\/ads\.example\/(a{1,3}|b)`,
			ok:   false,
		},
		{
			name: "unknown option",
			line: `URL-REGEX,^https?:\/\/ads\.example\/,MAGIC`,
			ok:   false,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			err := ValidateSurgeRulesetLine(test.line, true)
			if test.ok && err != nil {
				t.Fatalf("expected valid rule, got %s", *err)
			}
			if !test.ok && err == nil {
				t.Fatal("expected validation failure")
			}
		})
	}
}
