import sys
from pathlib import Path

sys.path.insert(0, "/Users/nyamiiko/Downloads/GitHub/script_hub/scripts")

from hub.common import safe_download
from hub.module_sanitizer import parse_module, merge_mitm_hosts

APPLE_SOURCES = [
    ("iRingo.Maps", "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Maps.sgmodule"),
    ("iRingo.WeatherKit", "https://github.com/NSRingo/WeatherKit/releases/latest/download/iRingo.WeatherKit.sgmodule"),
    ("iRingo.News", "https://github.com/NSRingo/News/releases/latest/download/iRingo.News.sgmodule"),
    ("iRingo.TV", "https://github.com/NSRingo/TV/releases/latest/download/iRingo.TV.sgmodule"),
    ("DualSubs.Universal", "https://github.com/DualSubs/Universal/releases/latest/download/DualSubs.Universal.sgmodule"),
]

merged_sections = {}
for label, url in APPLE_SOURCES:
    text = safe_download(url)
    meta, sections = parse_module(text)
    print(f"{label} has sections: {[s[0] for s in sections]}")
    for name, lines in sections:
        filtered_lines = [line for line in lines if line.strip()]
        if name in merged_sections and merged_sections[name] and filtered_lines:
            if merged_sections[name][-1] != "":
                merged_sections[name].append("")
        merged_sections.setdefault(name, []).extend(filtered_lines)

print("Merged sections keys:", list(merged_sections.keys()))
print("Merged MITM pre-processing:")
for line in merged_sections.get("MITM", []):
    print(f"  {line}")

section_list = [(name, merged_sections[name]) for name in merged_sections]
section_list = merge_mitm_hosts(section_list)
print("After merge_mitm_hosts:")
for name, lines in section_list:
    if name == "MITM":
        print(f"[{name}]")
        for line in lines:
            print(f"  {line}")
