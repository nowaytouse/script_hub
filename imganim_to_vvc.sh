#!/bin/bash

# 批量将指定文件夹内的动态图片 (GIF, APNG, WebP) 转换为 H.266/VVC 视频格式。
#
# 功能:
# - 递归查找指定目录下的所有文件，并使用 exiftool 检查其MIME类型，忽略扩展名。
# - 只处理识别出的动态图片 (image/gif, image/apng, image/webp)。
# - 使用 'ffmpeg' 和 'libvvenc' 编码器转换为 H.266/VVC 视频。
# - 尽力保留内部元数据，并完整保留系统文件时间戳。
# - 支持常规模式和原地转换模式。
#
# 使用方法:
# 1. 确保你已经安装了 ffmpeg (需编译支持 libvvenc) 和 exiftool。
#    - 在 macOS 上: brew install ffmpeg exiftool
#    - 注意: Homebrew 的 ffmpeg 可能不自带 libvvenc，可能需要手动编译或使用其他源。
# 2. 将此脚本赋予执行权限: chmod +x imganim_to_vvc.sh
# 3. 运行脚本:
#    - 常规模式 (创建新的 .mp4 文件):
#      ./imganim_to_vvc.sh /path/to/your/images
#    - 原地转换模式 (成功后用 .mp4 替换原始图片):
#      ./imganim_to_vvc.sh --in-place /path/to/your/images

# --- 默认值和参数解析 ---
IN_PLACE=false
TARGET_DIR=""

for arg in "$@"; do
  case $arg in
    --in-place)
      IN_PLACE=true
      shift
      ;;
    *)
      TARGET_DIR="$arg"
      ;;
  esac
done

# --- 检查依赖和参数 ---
if ! command -v ffmpeg &> /dev/null; then
    echo "错误: ffmpeg 命令未找到。"
    echo "请先安装 ffmpeg。在 macOS 上可以运行: brew install ffmpeg"
    exit 1
fi
if ! command -v exiftool &> /dev/null; then
    echo "错误: exiftool 命令未找到。"
    echo "请先安装 exiftool。在 macOS 上可以运行: brew install exiftool"
    exit 1
fi

if [ -z "$TARGET_DIR" ]; then
    echo "错误: 未指定目标文件夹路径。"
    echo "用法: $0 [--in-place] <目标文件夹路径>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    echo "错误: 目录 '$TARGET_DIR' 不存在。"
    exit 1
fi

# --- 安全检查 ---
if [ "$IN_PLACE" = true ]; then
    REAL_TARGET_DIR=""
    if command -v realpath &> /dev/null; then
        REAL_TARGET_DIR=$(realpath "$TARGET_DIR")
    else
        REAL_TARGET_DIR=$(cd "$TARGET_DIR"; pwd)
    fi

    FORBIDDEN_PATHS=("/" "/etc" "/bin" "/usr" "/System" "$HOME")

    for forbidden in "${FORBIDDEN_PATHS[@]}"; do
        if [ "$REAL_TARGET_DIR" = "$forbidden" ] || [[ "$REAL_TARGET_DIR" == "$forbidden/"* ]]; then
            echo "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
            echo "!!!                        安全警告                        !!!"
            echo "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
            echo "错误: 检测到危险操作！"
            echo "您正试图在受保护的系统目录 ($forbidden) 中执行原地替换操作。"
            echo "为了您的系统安全，此操作已被强制禁止。"
            echo "请选择一个普通的用户目录来执行此操作。"
            exit 1
        fi
    done
fi

echo "将在 '$TARGET_DIR' 文件夹中查找动态图片并转换为 H.266 (VVC) 视频..."
if [ "$IN_PLACE" = true ]; then
  echo "警告: 已启用 --in-place 模式，成功转换后将删除原始动态图片。"
fi

# --- 主逻辑 ---
find "$TARGET_DIR" -type f -print0 | while IFS= read -r -d $'\0' file; do
    
    MIME_TYPE=$(exiftool -MIMEType -b "$file")

    case "$MIME_TYPE" in
        "image/gif"|"image/apng"|"image/webp")
            echo "--------------------------------------------------"
            echo "发现动态图片: $file (类型: $MIME_TYPE)"
            
            output_mp4="${file%.*}.mp4"

            if [ "$IN_PLACE" = true ]; then
                # --- 原地转换逻辑 ---
                temp_mp4="${file}.mp4.tmp"
                echo "步骤 1/3: 正在转换为临时文件: $temp_mp4"
                
                ffmpeg -v warning -i "$file" -c:v libvvenc -crf 28 -pix_fmt yuv420p -y "$temp_mp4"
                
                if [ $? -eq 0 ]; then
                    echo "步骤 2/3: 正在从源文件迁移元数据..."
                    exiftool -tagsfromfile "$file" -all:all -overwrite_original "$temp_mp4" > /dev/null 2>&1
                    
                    echo "步骤 3/3: 正在同步时间戳并替换文件..."
                    touch -r "$file" "$temp_mp4"
                    
                    mv "$temp_mp4" "$output_mp4"
                    rm "$file"
                    
                    echo "完成: '$file' -> '$output_mp4'"
                else
                    echo "错误: 转换 '$file' 失败。临时文件将被删除。"
                    rm -f "$temp_mp4"
                fi
            else
                # --- 常规模式逻辑 ---
                if [ -f "$output_mp4" ]; then
                    echo "跳过: '$output_mp4' 已存在。"
                    continue
                fi

                echo "步骤 1/2: 正在转换 -> '$output_mp4'"
                ffmpeg -v warning -i "$file" -c:v libvvenc -crf 28 -pix_fmt yuv420p -y "$output_mp4"
                
                if [ $? -eq 0 ]; then
                    echo "步骤 2/2: 正在同步元数据和时间戳..."
                    exiftool -tagsfromfile "$file" -all:all -overwrite_original "$output_mp4" > /dev/null 2>&1
                    touch -r "$file" "$output_mp4"
                    echo "转换成功。"
                else
                    echo "错误: 转换 '$file' 失败。"
                fi
            fi
            ;;
        *)
            # 非目标文件，静默忽略
            ;;
    esac
done

echo "=========================================="
echo "所有文件处理完毕。"
echo "=========================================="
