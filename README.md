# Script Hub

面向 Surge、Shadowrocket、Loon 与 sing-box 的规则集和功能模块仓库。更新、网络抓取、合并、格式转换、校验与发布均由 Rust 完成。

## 广告拦截体系

- **PROMAX** 是唯一推荐安装的广告拦截模块：外部规则按用途分片，并合并经筛选的 URL Rewrite、Map Local、Body Rewrite、Script 与 MITM 功能段。
- **分类隔离**：Advertising、Privacy、Security、AntiAD 与各 ThreatIntel 类别由源清单显式声明；编译器不会按 URL 猜分类。
- **误屏蔽防护**：正常媒体、受保护服务、无对应请求规则的 MITM 主机会进入 `rulesets/AdBlock/quarantine.json`，不会直接发布。
- **旧 Helper**：原 AdBlock Helper 会全局注入 DNS 和大量 Host，现仅保留无操作占位以兼容旧订阅链接；请勿与 PROMAX 重复安装。端口阻断仍是独立、按需安装的模块。
- **旧 PROMAX 链接保持兼容**：
  `https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/surge/head_expanse/github/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule`

规则索引与分片说明见 `rulesets/AdBlock/README.md` 和 `rulesets/AdBlock/catalog.json`。

## 功能模块

- `modules/surge/amplify_nexus/`：可独立安装的增强、翻译、解锁与工具合集。
- `modules/shadowrocket/amplify_nexus/`：由 Rust 从已校验的 Surge 合集同步生成。
- `modules/source/local/`、`rulesets/Sources/LocalModules/`：编译输入，不应直接当作公开安装目录。
- 混合模块只把明确的广告行拆入 PROMAX；非广告增强功能继续留在独立模块中。
- 模块安装助手：`modules/helper/surge_module_helper.html`。其目录、兼容性和 Shadowrocket 状态均在同一次 Rust 更新中确定性刷新。

Apple 合集从 NSRingo 各官方 GitHub Release 的 `latest` 资源生成；WeatherKit、Maps、News、TV 与 DualSubs 不从公告文本或第三方转载页抓取。

## Rust 更新入口

```bash
cargo run --release --manifest-path create/processor/Cargo.toml --bin update -- \
  --root . --execute --singbox /path/to/sing-box

cargo run --manifest-path create/processor/Cargo.toml --bin validate -- --root .
```

`--quick` 仅跳过可选的功能模块上游刷新，PROMAX 规则仍会完整抓取、编译和验证。生成器会保留仅日期、注释、排版或顺序变化的旧文件，避免没有实质更新时产生自动提交。

规则源在 `rulesets/Sources/Links/AdBlock_sources.list` 中以 `SOURCE|POLICY|KIND|CATEGORY` 明确登记；脚本/重写源在 `AdBlock_functional_sources.list` 中维护。项目自有更新流程不再使用 Go、Python 或 Shell 脚本。
