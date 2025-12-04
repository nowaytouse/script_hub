#!/bin/bash

# 精简测试版：仅转换 MP4→WebP 无损，无复杂检查

set -e

INPUT_DIR="$1"

if [ -z "$INPUT_DIR" ] || [ ! -d "$INPUT_DIR" ]; then
    echo "❌ Usage: $0 <directory>"
    exit 1
fi

echo "🎬 Starting optimized MP4→WebP conversion..."
echo "📁 Directory: $INPUT_DIR"
echo ""

cd "$INPUT_DIR"

count=0
# 找到所有 MP4 文件
while IFS= read -r -d '' file; do
    ((count++)) || true
    filename=$(basename "$file")
    output="${file%.*}.webp"
    
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🎬 [$count] Converting: $filename"
    
    # 获取 FPS
    fps=$(ffprobe -v error -select_streams v:0 \
        -show_entries stream=r_frame_rate \
        -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null || echo "30")
    
    echo "  🎞️  FPS: $fps"
    
    # 直接转换
    echo "  🔄 Converting to lossless WebP..."
    start=$(date +%s)
    
    if ffmpeg -loglevel warning -stats -i "$file" \
        -c:v libwebp \
        -lossless 1 \
        -quality 100 \
        -compression_level 4 \
        -preset picture \
        -loop 0 \
        -vsync cfr \
        -r "$fps" \
        -an \
        -y "$output" 2>&1 | grep -i "frame=" | tail -1; then
        
        end=$(date +%s)
        elapsed=$((end - start))
        
        # 检查文件存在且非空
        if [ -f "$output" ] && [ -s "$output" ]; then
            size=$(stat -f%z "$output" 2>/dev/null || stat -c%s "$output")
            size_mb=$((size / 1024 / 1024))
            
            echo "  ✅ Done in ${elapsed}s → ${size_mb}MB"
            
            # 保留时间戳
            touch -r "$file" "$output"
            
            # 删除原文件
            rm "$file"
        else
            echo "  ❌ Conversion failed (output missing or empty)"
        fi
    else
        echo "  ❌ ffmpeg failed"
    fi
    
    echo ""
done < <(find . -maxdepth 1 -type f -iname "*.mp4" -print0)

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Processed $count MP4 file(s)"
