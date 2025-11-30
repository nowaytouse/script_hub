#!/bin/bash

# 批量将指定文件夹内的 JPEG 图片转换为 JXL 格式
#
# 功能:
# - 递归查找指定目录下的所有 .jpg, .jpeg 文件。
# - 使用 'cjxl' 进行高质量有损压缩。
# - 完整保留系统文件时间戳。
# - 支持常规模式和原地转换模式。
#
# 使用方法:
# 1. 确保你已经安装了 jpeg-xl (https://github.com/libjxl/libjxl)。
#    - 在 macOS 上: brew install jpeg-xl
# 2. 将此脚本赋予执行权限: chmod +x jpeg_to_jxl.sh
# 3. 运行脚本:
#    - 常规模式 (创建新的 .jxl 文件):
#      ./jpeg_to_jxl.sh /path/to/your/images
#    - 原地转换模式 (成功后用 .jxl 替换 .jpeg):
#      ./jpeg_to_jxl.sh --in-place /path/to/your/images

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
if ! command -v cjxl &> /dev/null; then
    echo "错误: cjxl 命令未找到。"
    echo "请先安装 jpeg-xl。在 macOS 上可以运行: brew install jpeg-xl"
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
    # 获取目标目录的真实绝对路径
    if command -v realpath &> /dev/null; then
        REAL_TARGET_DIR=$(realpath "$TARGET_DIR")
    else
        # realpath 的备用方案
        REAL_TARGET_DIR=$(cd "$TARGET_DIR"; pwd)
    fi

    # 定义危险目录列表
    FORBIDDEN_PATHS=("/" "/etc" "/bin" "/usr" "/System" "$HOME")

    for forbidden in "${FORBIDDEN_PATHS[@]}"; do
        # 检查真实路径是否与危险目录完全相同，或者是否是危险目录的一个子目录
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


echo "将在 '$TARGET_DIR' 文件夹中查找 JPEG 文件并转换为 JXL..."
if [ "$IN_PLACE" = true ]; then
  echo "警告: 已启用 --in-place 模式，成功转换后将删除原始 JPEG 文件。"
fi

# --- 主逻辑 ---
find "$TARGET_DIR" -type f \( -iname "*.jpg" -o -iname "*.jpeg" \) -print0 | while IFS= read -r -d $'\0' jpeg_file; do
    echo "--------------------------------------------------"
    echo "处理文件: $jpeg_file"
    
    # .jpeg or .jpg -> .jxl
    output_jxl="${jpeg_file%.*}.jxl"

    if [ "$IN_PLACE" = true ]; then
        # --- 原地转换逻辑 ---
        temp_jxl="${jpeg_file}.jxl.tmp"
        echo "正在转换为临时文件: $temp_jxl"
        
        # 使用 -d 1 进行高质量有损压缩
        cjxl "$jpeg_file" "$temp_jxl" -d 1
        
        if [ $? -eq 0 ]; then
            # 验证成功，复制时间戳
            echo "转换成功。正在同步时间戳并替换文件..."
            touch -r "$jpeg_file" "$temp_jxl"
            
            # 替换原始文件
            mv "$temp_jxl" "$output_jxl"
            rm "$jpeg_file"
            
            echo "完成: '$jpeg_file' -> '$output_jxl'"
        else
            echo "错误: 转换 '$jpeg_file' 失败。临时文件将被删除。"
            rm -f "$temp_jxl"
        fi
    else
        # --- 常规模式逻辑 ---
        if [ -f "$output_jxl" ]; then
            echo "跳过: '$output_jxl' 已存在。"
            continue
        fi

        echo "正在转换 -> '$output_jxl'"
        cjxl "$jpeg_file" "$output_jxl" -d 1
        
        if [ $? -eq 0 ]; then
            # 转换成功，复制时间戳
            touch -r "$jpeg_file" "$output_jxl"
            echo "转换成功，已同步时间戳。"
        else
            echo "错误: 转换 '$jpeg_file' 失败。"
        fi
    fi
done

echo "=========================================="
echo "所有 JPEG 文件处理完毕。"
echo "=========================================="
