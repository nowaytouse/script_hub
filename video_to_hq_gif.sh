#!/bin/bash

# 批量将指定文件夹内的视频文件转换为极高质量的 GIF。
#
# 功能:
# - 递归查找指定目录下的视频文件 (.mp4, .mov, .mkv, .avi, .webm)。
# - 使用 ffmpeg 两步法 (生成调色板 + 使用调色板) 创建极高质量 GIF。
#   - 支持自定义帧率和分辨率。
#   - 采用先进的抖动算法 (Bayer) 优化色彩过渡。
# - 尝试保留视频的内部元数据。
# - 完整保留系统文件时间戳。
# - 支持常规模式和删除源文件模式。
#
# 使用方法:
# 1. 确保你已经安装了 ffmpeg。
#    - 在 macOS 上: brew install ffmpeg
# 2. 将此脚本赋予执行权限: chmod +x video_to_hq_gif.sh
# 3. 运行脚本:
#    - 常规模式 (保留原始视频):
#      ./video_to_hq_gif.sh /path/to/your/videos
#    - 清理模式 (成功后删除原始视频):
#      ./video_to_hq_gif.sh --delete-source /path/to/your/videos
#    - 自定义帧率和宽度:
#      ./video_to_hq_gif.sh -r 24 -s 720 /path/to/your/videos
#
# 参数:
#   -r <fps>          : 设置输出 GIF 的帧率 (默认: 15)。
#   -s <width>        : 设置输出 GIF 的宽度，高度自动等比例缩放 (默认: 540)。
#   --delete-source   : 成功转换后删除原始视频文件。
#   <目标文件夹路径>  : 包含视频文件的目标文件夹。

# --- 默认值和参数解析 ---
DEFAULT_FPS=15
DEFAULT_SCALE=540 # GIF宽度，高度自动缩放

FPS=$DEFAULT_FPS
SCALE=$DEFAULT_SCALE
DELETE_SOURCE=false
TARGET_DIR=""

# 解析命令行参数
while (( "$#" )); do
  case "$1" in
    -r|--fps)
      if [ -n "$2" ] && ! [[ "$2" =~ ^- ]]; then
        FPS="$2"
        shift 2
      else
        echo "错误: -r/--fps 缺少值。" >&2
        exit 1
      fi
      ;;
    -s|--scale)
      if [ -n "$2" ] && ! [[ "$2" =~ ^- ]]; then
        SCALE="$2"
        shift 2
      else
        echo "错误: -s/--scale 缺少值。" >&2
        exit 1
      fi
      ;;
    --delete-source)
      DELETE_SOURCE=true
      shift
      ;;
    *) # 剩余的参数视为目标目录
      TARGET_DIR="$1"
      shift
      ;;
  esac
done

# --- 检查依赖和参数 ---
if ! command -v ffmpeg &> /dev/null; then
    echo "错误: ffmpeg 命令未找到。"
    echo "请先安装 ffmpeg。在 macOS 上可以运行: brew install ffmpeg"
    exit 1
fi

if [ -z "$TARGET_DIR" ]; then
    echo "错误: 未指定目标文件夹路径。"
    echo "用法: $0 [-r <fps>] [-s <width>] [--delete-source] <目标文件夹路径>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    echo "错误: 目录 '$TARGET_DIR' 不存在。"
    exit 1
fi

# --- 安全检查 ---
if [ "$DELETE_SOURCE" = true ]; then
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
            echo "您正试图在受保护的系统目录 ($forbidden) 中执行删除源文件操作。"
            echo "为了您的系统安全，此操作已被强制禁止。"
            echo "请选择一个普通的用户目录来执行此操作。"
            exit 1
        fi
    done
fi

echo "将在 '$TARGET_DIR' 文件夹中查找视频文件并转换为极高质量 GIF..."
echo "输出帧率: $FPS, 输出宽度: $SCALE"
if [ "$DELETE_SOURCE" = true ]; then
  echo "警告: 已启用 --delete-source 模式，成功转换后将删除原始视频文件。"
fi

# --- 主逻辑 ---
# 识别常见视频文件类型
find "$TARGET_DIR" -type f \( -iname "*.mp4" -o -iname "*.mov" -o -iname "*.mkv" -o -iname "*.avi" -o -iname "*.webm" \) -print0 | while IFS= read -r -d $'\0' video_file; do
    echo "--------------------------------------------------"
    echo "处理视频: $video_file"
    
    output_gif="${video_file%.*}.gif"

    if [ -f "$output_gif" ]; then
        echo "跳过: '$output_gif' 已存在。"
        continue
    fi

    palette="/tmp/palette_$(basename "$video_file").png"
    # 改进的 dithering 算法，例如 bayer:bayer_scale=5
    filters="fps=$FPS,scale=$SCALE:-1:flags=lanczos:force_original_aspect_ratio=disable"
    palette_filters="$filters,palettegen=stats_mode=diff:dither=bayer:bayer_scale=5"

    echo "步骤 1/2: 生成优化调色板..."
    # 使用 -loglevel error 减少 ffmpeg 自身的输出噪音
    ffmpeg -loglevel error -i "$video_file" -vf "$palette_filters" -y "$palette"
    
    if [ $? -ne 0 ]; then
        echo "错误: 生成调色板失败: '$video_file'"
        rm -f "$palette"
        continue
    fi

    echo "步骤 2/2: 使用调色板创建 GIF -> '$output_gif'"
    ffmpeg -loglevel error -i "$video_file" -i "$palette" -lavfi "$filters [x]; [x][1:v]paletteuse=dither=bayer:bayer_scale=5" -map_metadata 0 -y "$output_gif"

    if [ $? -eq 0 ]; then
        # 成功，同步时间戳
        touch -r "$video_file" "$output_gif"
        echo "成功创建 GIF，已同步时间戳。"

        if [ "$DELETE_SOURCE" = true ]; then
            rm "$video_file"
            echo "清理: 已删除源文件 '$video_file'"
        fi
    else
        echo "错误: 创建 GIF 失败: '$video_file'"
    fi
    
    # 清理调色板文件
    rm -f "$palette"
done

echo "=========================================="
echo "所有视频文件处理完毕。"
echo "=========================================="
