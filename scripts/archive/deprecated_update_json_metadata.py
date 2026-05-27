import json
import os

json_path = "/Users/nyamiiko/Downloads/GitHub/script_hub/module/modules_compatibility.json"

with open(json_path, "r", encoding="utf-8") as f:
    data = json.load(f)

# Modules that were merged into PROMAX (names from the JSON)
merged_names = {
    "去广告｜中国广电",
    "12306去广告",
    "123云盘",
    "IT之家去广告",
    "Jump去广告",
    "QQ音乐去广告",
    "RedNote",
    "Reddit 去广告",
    "Spotify去广告",
    "微博去广告",
    "京东去广告",
    "哔哩哔哩漫画去广告",
    "夸克去广告",
    "小宇宙去广告",
    "小红书去广告",
    "微博去广告&净化",
    "拼多多去广告",
    "淘宝去广告",
    "滴滴出行去广告",
    "百度网盘去广告",
    "知乎去广告",
    "菜鸟去广告",
    "闲鱼去广告",
    "阿里云盘去广告",
    "AWAvenue Ads Rule",
    "高德地图去广告"
}

new_surge_only = []
for m in data["modules"]["surge_only"]:
    if m["name"] == "🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style)":
        m["name"] = "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style)"
        new_surge_only.append(m)
    elif m["name"] in merged_names:
        continue
    else:
        new_surge_only.append(m)

data["modules"]["surge_only"] = new_surge_only
data["total"] = len(data["modules"]["compatible"]) + len(data["modules"]["surge_only"])

with open(json_path, "w", encoding="utf-8") as f:
    json.dump(data, f, ensure_ascii=False, indent=2)

print("Updated modules_compatibility.json")
