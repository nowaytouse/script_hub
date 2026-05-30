# Vendor snapshots (offline fallback)

Some upstream hosts (notably `yfamilys.com`) return HTTP errors to GitHub Actions / datacenter IPs.  
Committed files here keep CI and local runs reliable when live download fails.

| File | Upstream URL |
|------|----------------|
| `adultraplus.sgmodule` | https://yfamilys.com/module/adultraplus.sgmodule |
| `bili.module` | https://yfamilys.com/module/bili.module |
| `yfamilys_Kemono.list` | https://yfamilys.com/rule/Kemono.list |
| `yfamilys_Cloudflare.list` | https://yfamilys.com/rule/Cloudflare.list |

Refresh when upstream is reachable:

```bash
curl -fL -A "Mozilla/5.0" -o ruleset/Sources/vendor/adultraplus.sgmodule \
  "https://yfamilys.com/module/adultraplus.sgmodule"
# repeat for other URLs in the table
```

`scripts/lib/common.py` also uses `.cache/http/` and these snapshots automatically for matching URLs.
