#!/bin/bash

# 定义源文件和目标目录
SOURCE_MODULE="module/surge(main)/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
TARGET_DIR="$HOME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"

# 获取脚本所在目录的绝对路径
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# 检查源文件是否存在
if [ ! -f "$PROJECT_ROOT/$SOURCE_MODULE" ]; then
    echo "Error: Source module not found at $PROJECT_ROOT/$SOURCE_MODULE"
    exit 1
fi

# 检查目标目录是否存在，如果不存在则尝试创建（虽然通常 iCloud 目录由系统管理）
if [ ! -d "$TARGET_DIR" ]; then
    echo "Warning: Target directory $TARGET_DIR does not exist."
    echo "Attempting to create..."
    mkdir -p "$TARGET_DIR"
fi

# 复制文件
cp "$PROJECT_ROOT/$SOURCE_MODULE" "$TARGET_DIR/"

if [ $? -eq 0 ]; then
    echo "Successfully synced module to Shadowrocket."
else
    echo "Failed to sync module."
    exit 1
fi
