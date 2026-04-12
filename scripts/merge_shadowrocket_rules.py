#!/usr/bin/env python3
"""
merge_shadowrocket_rules.py
----------------------------
Categorises every rule in ruleset/shadowrocket.list into the appropriate
Surge(Shadowrocket) rule-set file and performs incremental, deduplicated merges.

Category mapping:
  Apple Intelligence / Siri relay  -> Apple.list  (PROXY)
  Copilot / AI                     -> AI.list      (PROXY)
  Baidu / iQIYI / CN direct        -> Direct.list  (DIRECT)
  Alibaba / CN direct              -> Direct.list  (DIRECT)
  General accelerated direct sites -> Direct.list  (DIRECT)
  Apple resources (direct)         -> Direct.list  (DIRECT)  *Apple relay already in Apple.list*
  Apple Intelligence relay (PROXY) -> Apple.list  (PROXY)
  LINE                             -> SocialMedia.list (PROXY)
  Google                           -> Google.list  (PROXY)
  Clubhouse                        -> SocialMedia.list (PROXY)
  Top blocked sites                -> GlobalProxy.list (PROXY)
  SoundCloud                       -> GlobalProxy.list (PROXY)
  ChatGPT / OpenAI                 -> AI.list      (PROXY)
  Telegram                         -> GlobalProxy.list (PROXY)
  DNS Leak / VPN                   -> GlobalProxy.list (PROXY)
  LAN                              -> Direct.list  (DIRECT)
  Blizzard / Battle.net            -> Gaming.list  (PROXY)
  Steam (all steamXXX DIRECT)      -> Direct.list  (DIRECT)  *Steam CDN is China-accelerated*
  m-team.cc                        -> GlobalProxy.list (PROXY)
  teamviewer.com                   -> GlobalProxy.list (PROXY)
  getpricetag.com                  -> GlobalProxy.list (PROXY)
  sciencemag.org                   -> GlobalProxy.list (PROXY)
  v2ex.com                         -> GlobalProxy.list (PROXY)
"""

import os, re
from datetime import datetime

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SRC  = os.path.join(ROOT, "ruleset", "shadowrocket.list")
DST  = os.path.join(ROOT, "ruleset", "Surge(Shadowkroket)")

# ─── helpers ──────────────────────────────────────────────────────────────────

def read_rules(path):
    """Return a set of bare rule strings (no policy suffix) from a Surge list file."""
    rules = set()
    if not os.path.exists(path):
        return rules
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            # Strip trailing ,PROXY / ,DIRECT / ,REJECT etc.
            parts = line.rsplit(",", 1)
            if len(parts) == 2 and parts[1].strip().split()[0] in (
                "PROXY", "DIRECT", "REJECT", "REJECT-NO-DROP", "REJECT-DROP"
            ):
                rules.add(parts[0].strip())
            else:
                rules.add(line)
    return rules


def read_full_file(path):
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8") as f:
        return f.read()


def write_file(path, content):
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)


HEADER_RE = re.compile(r"#\s*Rules:\s*\d+")
DATE_RE   = re.compile(r"#\s*Updated:\s*[\d\- :]+")


def update_header(content, rule_count):
    """Patch the '# Rules: N' and '# Updated: …' lines in the header."""
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    content = HEADER_RE.sub(f"# Rules: {rule_count}", content)
    content = DATE_RE.sub(f"# Updated: {now}", content)
    return content


def inject_rules_into_file(dest_path, new_rules, policy, ruleset_name, description):
    """
    Merge *new_rules* (list of bare rule strings) into *dest_path*.
    Rules already present are skipped (deduplication).
    The file header's Rules count and Updated timestamp are refreshed.
    If the file does not exist a brand-new one is created.
    """
    existing_bare = read_rules(dest_path)
    to_add = [r for r in new_rules if r not in existing_bare]
    if not to_add:
        print(f"  [skip] {os.path.basename(dest_path)}: nothing new to add")
        return 0

    # Build the lines to append
    section_lines = []
    section_lines.append(f"\n# ── merged from shadowrocket.list ({datetime.now().strftime('%Y-%m-%d')}) ──")
    for r in sorted(to_add):
        section_lines.append(f"{r},{policy}")

    full = read_full_file(dest_path)

    if not full.strip():
        # Create file from scratch
        now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        header = (
            f"# {'═'*63}\n"
            f"# Ruleset: {ruleset_name}\n"
            f"# Policy: {policy}\n"
            f"# Node: Any\n"
            f"# Description: {description}\n"
            f"# Rules: 0\n"
            f"# Updated: {now}\n"
            f"# {'═'*63}\n\n"
        )
        full = header

    full = full.rstrip("\n") + "\n" + "\n".join(section_lines) + "\n"

    # Recount rules (non-comment, non-empty lines)
    rule_count = sum(
        1 for l in full.splitlines()
        if l.strip() and not l.strip().startswith("#")
    )
    full = update_header(full, rule_count)
    write_file(dest_path, full)
    print(f"  [+{len(to_add):3d}] {os.path.basename(dest_path)}")
    return len(to_add)


# ─── parse source ──────────────────────────────────────────────────────────────

def parse_shadowrocket_list(path):
    """
    Return dict: category_name -> [(bare_rule, policy), ...]
    Categories correspond to Surge(Shadowrocket) files.
    """
    categories = {
        "Apple":       [],   # PROXY – relay / AI
        "AI":          [],   # PROXY
        "Direct":      [],   # DIRECT
        "Google":      [],   # PROXY
        "SocialMedia": [],   # PROXY
        "Gaming":      [],   # PROXY  (Blizzard / Battle.net)
        "GlobalProxy": [],   # PROXY  (everything else that's PROXY)
    }

    # We'll track which section we're in by comment headers
    section = None
    SECTION_MAP = {
        "apple intelligence": "Apple",
        "siri":               "Apple",
        "copilot":            "AI",
        "baidu":              "Direct",
        "iqiyi":              "Direct",
        "alibaba":            "Direct",
        "accelerate":         "Direct",
        "apple resources":    "Direct",   # "remove these lines…"
        "line":               "SocialMedia",
        "google":             "Google",
        "clubhouse":          "SocialMedia",
        "top blocked":        "GlobalProxy",
        "soundcloud":         "GlobalProxy",
        "chatgpt":            "AI",
        "telegram":           "GlobalProxy",
        "dns leak":           "GlobalProxy",
        "lan":                "Direct",
    }

    PREFIXES = (
        "DOMAIN-SUFFIX,", "DOMAIN,", "DOMAIN-KEYWORD,",
        "IP-CIDR,", "IP-CIDR6,", "IP-ASN,",
        "USER-AGENT,", "GEOIP,",
    )

    def bare(line):
        """Strip ,POLICY from the end of a rule line."""
        parts = line.rsplit(",", 1)
        if len(parts) == 2 and parts[1].strip().split()[0] in (
            "PROXY", "DIRECT", "REJECT", "REJECT-NO-DROP"
        ):
            return parts[0].strip(), parts[1].strip().split()[0]
        return line, None

    with open(path, encoding="utf-8") as f:
        lines = f.readlines()

    for raw in lines:
        line = raw.strip()
        if not line:
            continue

        # Detect section from comment
        if line.startswith("#"):
            low = line.lstrip("# ").lower()
            for key, cat in SECTION_MAP.items():
                if key in low:
                    section = cat
                    break
            continue

        if not any(line.startswith(p) for p in PREFIXES):
            continue

        rule_bare, policy = bare(line)
        if policy is None:
            continue   # malformed line

        # Override section based on known special-case domains
        effective_section = section

        # Steam domains tagged DIRECT → Direct (China CDN acceleration)
        if "steam" in rule_bare.lower() and policy == "DIRECT":
            effective_section = "Direct"
        # Blizzard / Battle.net → Gaming
        elif any(x in rule_bare.lower() for x in ("blizzard", "battle.net")):
            effective_section = "Gaming"
        # m-team, teamviewer, sciencemag, v2ex, getpricetag → GlobalProxy
        elif any(x in rule_bare.lower() for x in (
            "m-team", "teamviewer", "sciencemag", "v2ex", "getpricetag"
        )):
            effective_section = "GlobalProxy"

        # Policy override: if rule says DIRECT but section is PROXY, trust the rule
        # (e.g., ykimg.com appears under "top blocked sites" but is DIRECT)
        if policy == "DIRECT":
            effective_section = "Direct"
        elif policy == "PROXY" and effective_section == "Direct":
            # Something is PROXY but section said Direct → GlobalProxy
            effective_section = "GlobalProxy"

        # USER-AGENT only goes to GlobalProxy or SocialMedia
        if rule_bare.startswith("USER-AGENT"):
            if effective_section not in ("SocialMedia",):
                effective_section = "GlobalProxy"

        if effective_section is None:
            effective_section = "GlobalProxy" if policy == "PROXY" else "Direct"

        categories[effective_section].append(rule_bare)

    return categories


# ─── main ──────────────────────────────────────────────────────────────────────

META = {
    "Apple":       ("PROXY",  "Apple services"),
    "AI":          ("PROXY",  "AI services (OpenAI/Claude/Gemini)"),
    "Direct":      ("DIRECT", "Unified direct rules (China, Processes, Downloads)"),
    "Google":      ("PROXY",  "Google services"),
    "SocialMedia": ("PROXY",  "Social media fallback"),
    "Gaming":      ("PROXY",  "Gaming platforms"),
    "GlobalProxy": ("PROXY",  "Global proxy fallback"),
}


def main():
    print(f"\n{'═'*60}")
    print("  merge_shadowrocket_rules.py")
    print(f"{'═'*60}\n")

    cats = parse_shadowrocket_list(SRC)
    total_added = 0

    for cat, rules in cats.items():
        if not rules:
            continue
        policy, desc = META[cat]
        dest = os.path.join(DST, f"{cat}.list")
        # Deduplicate within the batch itself
        unique_rules = list(dict.fromkeys(rules))
        added = inject_rules_into_file(dest, unique_rules, policy, cat, desc)
        total_added += added

    print(f"\n{'─'*60}")
    print(f"  Done.  Total new rules added: {total_added}")
    print(f"{'─'*60}\n")


if __name__ == "__main__":
    main()
