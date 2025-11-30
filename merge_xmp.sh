#!/bin/bash

# 将指定文件夹内媒体文件的 .xmp 元数据侧车文件合并回其主文件。
#
# 功能:
# - 递归查找指定目录下的所有 .xmp 文件。
# - 自动识别对应的媒体文件 (例如 a.jpg.xmp -> a.jpg; a.xmp -> a.jpg/a.cr2/a.mp4)。
# - 使用 ExifTool 将 .xmp 文件中的元数据完整写入媒体文件。
# - 保留媒体文件的原始文件修改时间戳。
# - 为被修改的媒体文件自动创建备份 (e.g., "filename.jpg_original")。
# - (可选) 成功合并后删除 .xmp 文件。
#
# 使用方法:
# 1. 确保你已经安装了 ExifTool。
#    - 在 macOS 上: brew install exiftool
#    - 官方网站: https://exiftool.org/
# 2. 将此脚本赋予执行权限: chmod +x merge_xmp.sh
# 3. 运行脚本:
#    - 普通模式: ./merge_xmp.sh /path/to/your/media
#    - 删除XMP模式: ./merge_xmp.sh --delete-xmp /path/to/your/media

# --- 默认值和参数解析 ---
DELETE_XMP=false
TARGET_DIR=""

# 解析命令行参数
for arg in "$@"; do
  case $arg in
    --delete-xmp)
      DELETE_XMP=true
      shift # 移除 --delete-xmp 从参数列表
      ;;
    *)
      # 假定剩下的参数是目录
      TARGET_DIR="$arg"
      ;;
  esac
done

# --- 检查依赖和参数 ---
if ! command -v exiftool &> /dev/null; then
    echo "错误: ExifTool 命令未找到。"
    echo "请先安装 ExifTool。在 macOS 上可以运行: brew install exiftool"
    exit 1
fi

if [ -z "$TARGET_DIR" ]; then
    echo "错误: 未指定目标文件夹路径。"
    echo "用法: $0 [--delete-xmp] <目标文件夹路径>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    echo "错误: 目录 '$TARGET_DIR' 不存在。"
    exit 1
fi

# --- 安全检查 ---
if [ "$DELETE_XMP" = true ]; then
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
            echo "您正试图在受保护的系统目录 ($forbidden) 中执行删除XMP文件操作。"
            echo "为了您的系统安全，此操作已被强制禁止。"
            echo "请选择一个普通的用户目录来执行此操作。"
            exit 1
        fi
    done
fi

echo "将在 '$TARGET_DIR' 文件夹中查找并合并 .xmp 文件..."
echo "注意: ExifTool 将为每个被修改的文件创建一个 '_original' 备份。"
if [ "$DELETE_XMP" = true ]; then
  echo "警告: 已启用 --delete-xmp 模式，成功合并后将删除 .xmp 文件。"
fi

SUCCESS_COUNT=0
FAIL_COUNT=0
SKIPPED_COUNT=0

# --- 主逻辑 ---
# 使用 find 命令查找所有 .xmp 文件，并用 null 字符分隔，以安全处理带空格的文件名
find "$TARGET_DIR" -type f -iname "*.xmp" -print0 | while IFS= read -r -d $'\0' xmp_file; do
    echo "--------------------------------------------------"
    echo "找到 XMP 文件: $xmp_file"

    # 移除 .xmp 后缀得到基本文件名
    base_name="${xmp_file%.*}"

    # 检查基本文件名是否存在 (e.g., photo.jpg for photo.jpg.xmp)
    if [ -f "$base_name" ]; then
        media_file="$base_name"
    else
        # 如果 photo.jpg 不存在，则可能是 photo.xmp -> photo.cr2 的情况
        # 移除 .xmp 后缀，然后寻找同名的其他文件
        base_name_no_ext="${xmp_file%.xmp}"
        
        # 使用 find 在同一目录下查找具有相同基本名称但不同扩展名的文件
        # -maxdepth 1 确保只在当前 .xmp 文件所在的目录查找
        media_file=$(find "$(dirname "$xmp_file")" -maxdepth 1 -type f -name "$(basename "$base_name_no_ext").*" ! -name "*.xmp" | head -n 1)

        if [ -z "$media_file" ]; then
             echo "警告: 未找到 '$xmp_file' 对应的媒体文件。跳过。"
             SKIPPED_COUNT=$((SKIPPED_COUNT + 1))
             continue
        fi
    fi

    echo "找到对应媒体文件: $media_file"

    # 执行合并操作
    # -tagsfromfile: 从指定的 .xmp 文件读取元数据
    # -all:all: 尝试写入所有可写的元数据标签
    # -P: 保留原始文件的文件系统修改时间戳
    # ExifTool 默认会创建备份文件 (filename_original)
    echo "正在合并元数据..."
    exiftool -P -tagsfromfile "$xmp_file" -all:all "$media_file"

    if [ $? -eq 0 ]; then
        echo "成功: 元数据已从 '$xmp_file' 合并到 '$media_file'"
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))

        if [ "$DELETE_XMP" = true ]; then
            rm "$xmp_file"
            echo "清理: 已删除 '$xmp_file'"
        fi
    else
        echo "错误: 合并 '$xmp_file' 到 '$media_file' 失败。"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
done

echo "=========================================="
echo "合并完成。"
echo "成功: $SUCCESS_COUNT"
echo "失败: $FAIL_COUNT"
echo "跳过: $SKIPPED_COUNT"
echo "=========================================="
