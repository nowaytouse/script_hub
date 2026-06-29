#!/usr/bin/env python3
"""Refresh rulesets/Sources/vendor from live yfamilys.com URLs."""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VENDOR = ROOT / "rulesets" / "Sources" / "vendor"
UA = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
)
PAIRS = [
    ("https://yfamilys.com/module/adultraplus.sgmodule", "adultraplus.sgmodule"),
    ("https://yfamilys.com/module/bili.module", "bili.module"),
    ("https://yfamilys.com/rule/Kemono.list", "yfamilys_Kemono.list"),
    ("https://yfamilys.com/rule/Cloudflare.list", "yfamilys_Cloudflare.list"),
]


def main() -> int:
    VENDOR.mkdir(parents=True, exist_ok=True)
    failed = 0
    for url, name in PAIRS:
        dest = VENDOR / name
        r = subprocess.run(
            ["curl", "-fL", "-m", "90", "-A", UA, "-o", str(dest), url],
            capture_output=True,
        )
        if r.returncode != 0:
            print(f"FAIL {name} ({url})", file=sys.stderr)
            failed += 1
        else:
            print(f"OK   {name} ({dest.stat().st_size} bytes)")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
