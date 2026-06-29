# Script Hub 一键更新指南

本项目当前提供两套完全等效的一键更新流：传统的 **Python 版本** 与重构后的 **Go 版本**。
两者的行为 1:1 完全一致，均支持模块合并、域名规则转换、广告屏蔽集成及上游规则同步。为防误触引发 Github 封禁（shadowban），所有下载和同步均已设定安全节流延迟（稳定大于速度）。

## 1. 使用 Go 语言版本 (推荐)

Go 语言版本具有无需安装第三方依赖、单文件运行、内存占用低等优点。当前最新版的 Go 程序已充分限制了并发速率，防止触发远程站点（如 GitHub Raw）的封禁策略。

### **如何运行：**
在终端中进入项目目录，并执行：
```bash
# 全量更新 + 并推送至 Github (适合每日自动发版)
cd create
go run ./cmd/main_update --execute

# 测试/局部更新 (不提交至 Github，本地处理)
cd create
go run ./cmd/main_update --quick
```

**命令行参数说明：**
- `--execute`: 允许脚本执行最终的 `git commit` 与 `git push`（真实发版）。如果未传递此参数，生成依然会在本地完成，但不会推送到远程。
- `--quick`: 最小化测试模式，跳过上游大量规则库的抓取（SyncSkk、SyncNexus等），仅合并主干组件，大幅减少执行耗时。

---

## 2. 使用 Python 语言版本 (经典)

Python 版本是原工程的经典方案，所有重写、正则表达式规则也已做到了高度优化。

### **如何运行：**
确保已安装 Python 环境（要求 `requests` 库），然后在终端中执行：
```bash
# 全量更新 + 推送至 Github
python3 create/scripts/pipeline/main_update.py --execute

# 测试/局部更新 (不提交至 Github，本地处理)
python3 create/scripts/pipeline/main_update.py --quick
```

**命令行参数说明：**
- `--execute`: 执行 `git push` 发布代码。
- `--quick`: 跳过全量抓取阶段。
- `--no-tests`: 强制跳过 QA 合规性前置测试。

---

## 3. 防护与合规性 (安全申明)

> [!WARNING]
> **切忌强行加速执行或多开脚本！**
> 因为本项目大量依赖 `raw.githubusercontent.com` 进行上游配置的下发，高频、无延迟的持续拉取会立即触发 GitHub 的 Anti-Abuse System（滥用拦截），导致你的账号被 Shadowban（即：你自己可见，其他人访问仓库内文件全是 404）。
> 当前在 Python 和 Go 版本中均已硬编码了 **3秒以上的轮询延迟**。请保持此设定：**稳定性大于速度**。

### QA 自动化校验
无论你使用 Go 还是 Python，每次运行主流程之前，系统会自动调用 `QA 质检流程`（检查 Surge / Shadowrocket 规则是否越界、URL-REGEX 是否合规、DOMAIN-REGEX 转换是否无损等），并在任何测试失败时直接**熔断中止**，绝不会将残次品推送到生产环境。
