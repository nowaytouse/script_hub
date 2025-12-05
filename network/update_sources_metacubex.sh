#!/bin/bash
# 更新Sources文件，添加MetaCubeX本地规则

SCRIPT_DIR="$(dirname "$0")"
SOURCES_DIR="${SCRIPT_DIR}/../../ruleset/Sources"
METACUBEX_DIR="${SCRIPT_DIR}/../../ruleset/MetaCubeX"

echo "=== 更新Sources文件，添加MetaCubeX本地规则 ==="

for pair in "Telegram:telegram" "Discord:discord" "Google:google" "Apple:apple" "Microsoft:microsoft" "Bilibili:bilibili" "AI:category-ai-cn" "Gaming:category-games" "AdBlock:category-ads-all" "Instagram:instagram" "Twitter:twitter" "Spotify:spotify" "Steam:steam" "YouTube:youtube" "GitHub:github" "Netflix:netflix" "TikTok:tiktok" "PayPal:paypal" "SocialMedia:facebook" "Reddit:reddit" "GlobalProxy:cloudflare"; do
    name="${pair%%:*}"
    metacubex="${pair##*:}"
    sources_file="${SOURCES_DIR}/${name}_sources.txt"
    metacubex_file="${METACUBEX_DIR}/MetaCubeX_${metacubex}.list"
    
    if [ -f "$sources_file" ] && [ -f "$metacubex_file" ]; then
        if ! grep -q "MetaCubeX" "$sources_file" 2>/dev/null; then
            echo "更新: $name"
            echo "" >> "$sources_file"
            echo "# MetaCubeX 本地规则 (从.srs反编译)" >> "$sources_file"
            echo "../MetaCubeX/MetaCubeX_${metacubex}.list" >> "$sources_file"
        else
            echo "跳过: $name (已包含MetaCubeX)"
        fi
    else
        echo "跳过: $name (文件不存在)"
    fi
done

echo "=== 完成 ==="
