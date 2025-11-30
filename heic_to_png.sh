#!/bin/bash

# 批量将指定文件夹内的 HEIC/HEIF 图片转换为数学无损的 PNG 格式。
#
# 功能:
# - 递归查找指定目录下的所有 .heic, .heif 文件。
# - 使用 'heif-convert' 进行高质量转换。
# - 使用 'exiftool' 从源文件复制所有元数据，确保元数据完整性。
# - 完整保留系统文件时间戳。
# - 支持常规模式和原地转换模式。
#
# 使用方法:
# 1. 确保你已经安装了 libheif 和 exiftool。
#    - 在 macOS 上: brew install libheif exiftool
# 2. 将此脚本赋予执行权限: chmod +x heic_to_png.sh
# 3. 运行脚本:
#    - 常规模式 (创建新的 .png 文件):
#      ./heic_to_png.sh /path/to/your/images
#    - 原地转换模式 (成功后用 .png 替换 .heic):
#      ./heic_to_png.sh --in-place /path/to/your/images

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
if ! command -v heif-convert &> /dev/null; then
    echo "错误: heif-convert 命令未找到。"
    echo "请先安装 libheif。在 macOS 上可以运行: brew install libheif"
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

echo "将在 '$TARGET_DIR' 文件夹中查找 HEIC/HEIF 文件并转换为无损 PNG..."
if [ "$IN_PLACE" = true ]; then
  echo "警告: 已启用 --in-place 模式，成功转换后将删除原始 HEIC/HEIF 文件。"
fi

# --- 主逻辑 ---
find "$TARGET_DIR" -type f \( -iname "*.heic" -o -iname "*.heif" \) -print0 | while IFS= read -r -d $'\0' heic_file; do
    echo "--------------------------------------------------"
    echo "处理文件: $heic_file"
    
    output_png="${heic_file%.*}.png"

    if [ "$IN_PLACE" = true ]; then
        # --- 原地转换逻辑 ---
        temp_png="${heic_file}.png.tmp"
        echo "步骤 1/3: 正在转换为临时文件: $temp_png"
        heif-convert "$heic_file" "$temp_png"
        
        if [ $? -eq 0 ]; then
            echo "步骤 2/3: 正在从源文件迁移完整元数据..."
            # -overwrite_original 在这里是安全的，因为它作用于 موقت 文件
            exiftool -tagsfromfile "$heic_file" -all:all -overwrite_original "$temp_png" > /dev/null
            
            echo "步骤 3/3: 正在同步时间戳并替换文件..."
            touch -r "$heic_file" "$temp_png"
            
            # 替换原始文件
            mv "$temp_png" "$output_png"
            rm "$heic_file"
            
            echo "完成: '$heic_file' -> '$output_png'"
        else
            echo "错误: 转换 '$heic_file' 失败。临时文件将被删除。"
            rm -f "$temp_png"
        fi
    else
        # --- 常规模式逻辑 ---
        if [ -f "$output_png" ]; then
            echo "跳过: '$output_png' 已存在。"
            continue
        fi

        echo "步骤 1/2: 正在转换 -> '$output_png'"
        heif-convert "$heic_file" "$output_png"
        
        if [ $? -eq 0 ]; then
            echo "步骤 2/2: 正在从源文件迁移完整元数据..."
            exiftool -tagsfromfile "$heic_file" -all:all -overwrite_original "$output_png" > /dev/null
            touch -r "$heic_file" "$output_png"
            echo "转换成功，已同步元数据和时间戳。"
        else
            echo "错误: 转换 '$heic_file' 失败。"
        fi
    fi
done

echo "=========================================="
echo "所有 HEIC/HEIF 文件处理完毕。"
echo "=========================================="
