#!/opt/homebrew/bin/bash

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║                                                                ║"
echo "║   清理 Git 历史中的敏感信息                                     ║"
echo "║                                                                ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

echo "⚠️  警告: 这将重写 Git 历史！"
echo "⚠️  所有协作者需要重新克隆仓库！"
echo ""
read -p "确认继续? (yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "已取消"
    exit 0
fi

echo ""
echo "🔄 清理 config.json 从所有历史..."

# 使用 git filter-branch 删除文件
git filter-branch --force --index-filter \
  "git rm --cached --ignore-unmatch config.json" \
  --prune-empty --tag-name-filter cat -- --all

echo ""
echo "🗑️  清理引用..."
rm -rf .git/refs/original/
git reflog expire --expire=now --all
git gc --prune=now --aggressive

echo ""
echo "✅ 清理完成！"
echo ""
echo "📤 强制推送到远程仓库:"
echo "   git push origin --force --all"
echo ""
echo "⚠️  注意: 所有协作者需要执行:"
echo "   git fetch origin"
echo "   git reset --hard origin/main"
