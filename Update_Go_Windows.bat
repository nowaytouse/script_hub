@echo off
chcp 65001 >nul
cd /d "%~dp0go_scripts"
echo 🚀 正在运行 Go 语言版一键更新 (全量执行)...
go run ./cmd/main_update --execute
echo ✅ 更新完成！请按任意键退出...
pause >nul
