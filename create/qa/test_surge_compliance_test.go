package qa

import "testing"

func TestSurgeComplianceRegression(t *testing.T) {
	if failed := RunTestSurgeCompliance(); failed != 0 {
		t.Fatalf("compliance regression failed: %d", failed)
	}
}
