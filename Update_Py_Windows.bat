@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo 🚀 正在运行 Python 语言版一键更新 (全量执行)...
python scripts\pipeline\main_update.py --execute
echo ✅ 更新完成！请按任意键退出...
pause >nul
