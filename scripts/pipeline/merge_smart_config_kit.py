"""
Merge Smart-Config-Kit supplemental rules into Script Hub rulesets.

Source rules live at:
  rulesets/Sources/custom/SmartConfigKit/shunt-rules/  (Passwall2 shunt-rules format)
  rulesets/Sources/custom/SmartConfigKit/UK.list        (Surge DOMAIN/KEYWORD format)
  rulesets/Sources/custom/SmartConfigKit/CiciAi.list   (Surge DOMAIN/KEYWORD format)

The former fork dependency (Smart-Config-Kit/) has been removed; all source
files are now vendored under rulesets/Sources/custom/SmartConfigKit/.
"""

import os
import re
from collections import defaultdict
from hub.common import Logger, get_project_root, write_file

ROOT = get_project_root()
SCK_DIR = os.path.join(ROOT, "rulesets/Sources/custom/SmartConfigKit/shunt-rules")
CUSTOM_DIR = os.path.join(ROOT, "rulesets/Sources/custom/SmartConfigKit")
SOURCES_DIR = os.path.join(ROOT, "rulesets/Sources/Links")

# Surge-format extra lists vendored inside custom/SmartConfigKit/
EXTRA_LISTS = {
    "UK.list": "StreamEU",
    "CiciAi.list": "AI",
}

# Mapping: SCK shunt-rules file → Script Hub target ruleset
MAPPING = {
    "01-ai-service.list": "AI",
    "02-crypto.list": "Binance",
    "03-payments.list": "PayPal",
    "04-email.list": "SocialMedia",
    "05-im.list": "SocialMedia",
    "06-social.list": "SocialMedia",
    "07-work.list": "GitHub",
    "08-cn-media.list": "Bilibili",
    "09-sea-media.list": "StreamHK",
    "10-us-media.list": "StreamUS",
    "11-hk-media.list": "StreamHK",
    "12-tw-media.list": "StreamTW",
    "13-jp-media.list": "StreamJP",
    "14-eu-media.list": "StreamEU",
    "15-cn-game.list": "Direct",
    "16-intl-game.list": "Gaming",
    "17-search.list": "Google",
    "18-dev.list": "GitHub",
    "19-microsoft.list": "Microsoft",
    "20-apple.list": "Apple",
    "21-download.list": "Direct",
    "22-cloud-cdn.list": "CDN",
    "24-cn-site.list": "Direct",
    "25-gfw.list": "GlobalProxy",
    "26-intl-site.list": "GlobalProxy",
    "27-final.list": "GlobalProxy",
    "28-ads.list": "AdBlock",
}

_SURGE_RULE = re.compile(
    r"^(DOMAIN(?:-SUFFIX|-KEYWORD|-WILDCARD)?|IP-CIDR6?|PROCESS-NAME),(.+)$",
    re.IGNORECASE,
)


def _parse_shunt(filepath: str) -> list[str]:
    """Parse Passwall2/Xray domain: / full: / keyword: syntax → Surge rules."""
    rules: list[str] = []
    if not os.path.exists(filepath):
        return rules
    with open(filepath, "r", encoding="utf-8") as f:
        for raw in f:
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("domain:"):
                rules.append(f"DOMAIN-SUFFIX,{line[7:]}")
            elif line.startswith("full:"):
                rules.append(f"DOMAIN,{line[5:]}")
            elif line.startswith("keyword:"):
                rules.append(f"DOMAIN-KEYWORD,{line[8:]}")
            # geosite:xxx — ignored; script_hub syncs geosite separately via MetaCubeX
    return rules


def _parse_surge_list(filepath: str) -> list[str]:
    """Parse a Surge-format .list file, keeping DOMAIN/IP/PROCESS lines."""
    rules: list[str] = []
    if not os.path.exists(filepath):
        return rules
    with open(filepath, "r", encoding="utf-8") as f:
        for raw in f:
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            if _SURGE_RULE.match(line):
                rules.append(line)
    return rules


def merge() -> None:
    Logger.section("Merging Smart-Config-Kit supplemental rules")
    os.makedirs(CUSTOM_DIR, exist_ok=True)

    merged: dict[str, dict] = defaultdict(lambda: {"rules": set(), "sources": []})

    # --- shunt-rules (Passwall2 format) ---
    for sck_file, target in MAPPING.items():
        path = os.path.join(SCK_DIR, sck_file)
        rules = _parse_shunt(path)
        if not rules:
            continue
        merged[target]["rules"].update(rules)
        merged[target]["sources"].append(sck_file)
        Logger.success(f"  {len(rules):4d} rules  {sck_file} → {target}")

    # --- extra Surge-format lists ---
    for list_file, target in EXTRA_LISTS.items():
        path = os.path.join(CUSTOM_DIR, list_file)
        rules = _parse_surge_list(path)
        if not rules:
            continue
        merged[target]["rules"].update(rules)
        merged[target]["sources"].append(list_file)
        Logger.success(f"  {len(rules):4d} rules  {list_file} → {target}")

    # --- write SCK_*.txt and link into *_sources.txt ---
    for target, payload in sorted(merged.items()):
        custom_file = f"SCK_{target}.txt"
        custom_path = os.path.join(CUSTOM_DIR, custom_file)

        header = "# Smart-Config-Kit supplement: " + ", ".join(payload["sources"]) + "\n"
        write_file(custom_path, header + "\n".join(sorted(payload["rules"])) + "\n")
        Logger.success(f"  Wrote {len(payload['rules'])} rules → {custom_file}")

        sources_file = os.path.join(SOURCES_DIR, f"{target}_sources.txt")
        if not os.path.exists(sources_file):
            continue
        with open(sources_file, "r", encoding="utf-8") as f:
            content = f.read()
        relative_link = f"../custom/SmartConfigKit/{custom_file}"
        if relative_link not in content:
            with open(sources_file, "a", encoding="utf-8") as f:
                f.write(f"\n# Smart-Config-Kit supplement\n{relative_link}\n")
            Logger.info(f"  Linked {custom_file} → {target}_sources.txt")


if __name__ == "__main__":
    merge()
