#!/bin/bash

# 将指定文件夹内的文件按每500MB的大小分割，打包成 .tar.gz 压缩包，
# 并上传到指定的 GitHub 仓库的 Releases 中。
#
# 使用方法:
# 1. 确保你已经安装了 GitHub CLI (gh)。
#    - 在 macOS 上: brew install gh
#    - 详细信息: https://cli.github.com/
# 2. 使用 'gh auth login' 命令进行认证。
# 3. 将此脚本赋予执行权限: chmod +x archive_and_upload.sh
# 4. 运行脚本: ./archive_and_upload.sh <源文件夹> <GitHub仓库OWNER/REPO>
#    例如: ./archive_and_upload.sh ./my_large_files my_username/my_repo

# --- 配置 ---
# 500 MB (in bytes)
MAX_SIZE=$((500 * 1024 * 1024))

# --- 检查依赖和参数 ---
if ! command -v gh &> /dev/null; then
    echo "错误: GitHub CLI (gh) 命令未找到。"
    echo "请先安装 gh。在 macOS 上可以运行: brew install gh"
    exit 1
fi

if [ -z "$1" ] || [ -z "$2" ]; then
    echo "用法: $0 <源文件夹> <GitHub仓库OWNER/REPO>"
    exit 1
fi

SOURCE_DIR=$1
REPO=$2

if [ ! -d "$SOURCE_DIR" ]; then
    echo "错误: 源文件夹 '$SOURCE_DIR' 不存在。"
    exit 1
fi

# --- 主逻辑 ---
echo "正在检查 GitHub 认证状态..."
if ! gh auth status &> /dev/null; then
    echo "错误: 未通过 GitHub CLI 认证。请运行 'gh auth login'。"
    exit 1
fi

echo "开始处理文件夹: $SOURCE_DIR"
echo "目标仓库: $REPO"

# 创建一个临时文件列表
TMP_FILE_LIST=$(mktemp)
# 创建一个临时文件用于存储 find 的输出，避免在管道中读取时产生子shell问题
FIND_OUTPUT=$(mktemp)

# 清理临时文件
trap 'rm -f "$TMP_FILE_LIST" "$FIND_OUTPUT"' EXIT

# 查找所有文件并存储路径和大小（以字节为单位）
find "$SOURCE_DIR" -type f -print0 | xargs -0 du -b > "$FIND_OUTPUT"

current_size=0
part=1

while read -r size file; do
    echo "$file" >> "$TMP_FILE_LIST"
    current_size=$((current_size + size))

    if (( current_size >= MAX_SIZE )); then
        ARCHIVE_NAME="archive_part_${part}.tar.gz"
        echo "创建压缩包 '$ARCHIVE_NAME' (大小: ~$(($current_size / 1024 / 1024)) MB)..."

        # 从 SOURCE_DIR 的父目录开始打包，以保留 SOURCE_DIR 本身的目录结构
        tar -czf "$ARCHIVE_NAME" --files-from="$TMP_FILE_LIST"
        
        if [ $? -eq 0 ]; then
            TAG="release-$(date +%Y%m%d%H%M%S)-part${part}"
            echo "正在创建 GitHub Release 并上传 '$ARCHIVE_NAME'..."
            gh release create "$TAG" "$ARCHIVE_NAME" --repo "$REPO" --title "自动归档 - Part ${part}" --notes "自动打包和上传的文件集合，Part ${part}。"
            if [ $? -ne 0 ]; then
                echo "错误: 上传到 GitHub Release 失败。"
                # 如果上传失败，可以选择不删除本地压缩包以便手动处理
                # exit 1
            else
                echo "上传成功，删除本地压缩包 '$ARCHIVE_NAME'。"
                rm "$ARCHIVE_NAME"
            fi
        else
            echo "错误: 创建压缩包 '$ARCHIVE_NAME' 失败。"
        fi

        # 重置
        > "$TMP_FILE_LIST"
        current_size=0
        part=$((part + 1))
    fi
# 使用 < 操作符重定向输入，以确保循环在当前 shell 中执行
done < <(awk '{print $1, $2}' "$FIND_OUTPUT")


# 处理剩余的文件（如果存在）
if [ -s "$TMP_FILE_LIST" ]; then
    ARCHIVE_NAME="archive_part_${part}.tar.gz"
    echo "创建最后一个压缩包 '$ARCHIVE_NAME' (大小: ~$(($current_size / 1024 / 1024)) MB)..."
    
    tar -czf "$ARCHIVE_NAME" --files-from="$TMP_FILE_LIST"

    if [ $? -eq 0 ]; then
        TAG="release-$(date +%Y%m%d%H%M%S)-part${part}"
        echo "正在创建 GitHub Release 并上传 '$ARCHIVE_NAME'..."
        gh release create "$TAG" "$ARCHIVE_NAME" --repo "$REPO" --title "自动归档 - Part ${part}" --notes "自动打包和上传的文件集合，Part ${part}。"
        if [ $? -ne 0 ]; then
            echo "错误: 上传到 GitHub Release 失败。"
        else
            echo "上传成功，删除本地压缩包 '$ARCHIVE_NAME'。"
            rm "$ARCHIVE_NAME"
        fi
    else
        echo "错误: 创建压缩包 '$ARCHIVE_NAME' 失败。"
    fi
fi

echo "所有文件处理完毕。"
