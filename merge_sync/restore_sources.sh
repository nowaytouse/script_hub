#!/bin/bash
# restore_sources.sh
# Re-creates missing *_sources.txt files with standard Blackmatrix7 URLs

LINKS_DIR="/Users/nyamiiko/Library/Mobile Documents/com~apple~CloudDocs/Application/script_hub/ruleset/Sources/Links"
mkdir -p "$LINKS_DIR"

declare -A SOURCES=(
    ["GlobalMedia"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/GlobalMedia/GlobalMedia.list"
    ["YouTube"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/YouTube/YouTube.list"
    ["Netflix"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Netflix/Netflix.list"
    ["Disney"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Disney/Disney.list"
    ["Spotify"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Spotify/Spotify.list"
    ["Telegram"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Telegram/Telegram.list"
    ["Twitter"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Twitter/Twitter.list"
    ["TikTok"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/TikTok/TikTok.list"
    ["Google"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Google/Google.list"
    ["Microsoft"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Microsoft/Microsoft.list"
    ["PayPal"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/PayPal/PayPal.list"
    ["Apple"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Apple/Apple.list"
    ["Steam"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Steam/Steam.list"
    ["Discord"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Discord/Discord.list"
    ["Bilibili"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Bilibili/Bilibili.list"
    ["ChinaIP"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/ChinaIP/ChinaIP.list"
    ["WeChat"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/WeChat/WeChat.list"
    ["Bing"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Bing/Bing.list"
    ["Reddit"]="https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Reddit/Reddit.list"
)

for name in "${!SOURCES[@]}"; do
    file="$LINKS_DIR/${name}_sources.txt"
    if [[ ! -f "$file" ]]; then
        echo "${SOURCES[$name]}" > "$file"
        echo "Created $file"
    else
        # Append if not present? No, overwrite for restoration.
        # But respect existing content if manual?
        if ! grep -q "blackmatrix7" "$file"; then
             echo "${SOURCES[$name]}" >> "$file"
             echo "Updated $file"
        fi
    fi
done
