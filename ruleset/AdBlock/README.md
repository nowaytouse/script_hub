# AdBlock 分片规则集

> 自动生成于 `2026-05-31 13:56:10` · 每片 ≤ 100,000 条 · 详见 [`catalog.json`](catalog.json)

## 用途分类

| 分类 | 说明 |
|------|------|
| **应用去广告** (`Local`) | 各 App 去广告域名规则（脚本/重写已并入 PROMAX，勿单独安装专项模块） |
| **通用广告** (`Advertising`) | blackmatrix7 / ACL4SSR / geekdada / 广告联盟等通用域名 |
| **隐私追踪** (`Privacy`) | Privacy / tracking-protection / 防追踪列表 |
| **安全威胁** (`Security`) | 劫持 / 钓鱼 / BanProgram / HTTPDNS / skk reject |
| **Anti-AD** (`AntiAD`) | privacy-protection-tools anti-AD 主列表与 discretion |
| **威胁情报·终极** (`ThreatIntel_Ultimate`) | HaGeZi DNS-Blocklists ultimate |
| **威胁情报·TIF** (`ThreatIntel_TIF`) | HaGeZi TIF medium 威胁情报 |
| **威胁情报·补充** (`ThreatIntel`) | HaGeZi / skk 其它威胁与钓鱼域名集 |
| **其他补充** (`Other`) | 社区 sgmodule / 未归类远程源 |

## 当前分片

| 用途 | 文件 | 规则数 | CDN |
|------|------|--------|-----|
| 应用去广告 | `ruleset/AdBlock/AdBlock_Local.list` | 98 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Local.list) |
| 通用广告 | `ruleset/AdBlock/AdBlock_Advertising_01.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Advertising_01.list) |
| 通用广告 | `ruleset/AdBlock/AdBlock_Advertising_02.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Advertising_02.list) |
| 通用广告 | `ruleset/AdBlock/AdBlock_Advertising_03.list` | 79,384 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Advertising_03.list) |
| 隐私追踪 | `ruleset/AdBlock/AdBlock_Privacy.list` | 5,588 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Privacy.list) |
| 安全威胁 | `ruleset/AdBlock/AdBlock_Security_01.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Security_01.list) |
| 安全威胁 | `ruleset/AdBlock/AdBlock_Security_02.list` | 26,388 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Security_02.list) |
| Anti-AD | `ruleset/AdBlock/AdBlock_AntiAD.list` | 6,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_AntiAD.list) |
| 威胁情报·终极 | `ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_01.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_01.list) |
| 威胁情报·终极 | `ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_02.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_02.list) |
| 威胁情报·终极 | `ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_03.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_03.list) |
| 威胁情报·终极 | `ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_04.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_04.list) |
| 威胁情报·终极 | `ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_05.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_05.list) |
| 威胁情报·终极 | `ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_06.list` | 100,000 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_06.list) |
| 威胁情报·终极 | `ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_07.list` | 61,072 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_ThreatIntel_Ultimate_07.list) |
| 其他补充 | `ruleset/AdBlock/AdBlock_Other.list` | 27,952 | [link](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Other.list) |

## 应用内脚本层（PROMAX 内嵌）

| 段落 | 行数 |
|------|------|
| `URL Rewrite` | 834 |
| `Map Local` | 1,944 |
| `Script` | 606 |
| `Body Rewrite` | 79 |
| `MITM` (hostname) | 1,010 |

## 安装模块（RULE-SET + 去广告 Script/Rewrite；解锁/增强请用 Amplify Nexus 独立模块）

`ruleset/Sources/LocalModules/*` 各 App 去广告逐文件并入 PROMAX，**勿单独安装**专项模块或已废弃的 narrow_pierce 副本。

### 🖥️ 桌面完整版（Full）
- **Surge PROMAX**: [https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/module/surge%28main%29/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/module/surge%28main%29/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule)
- **Shadowrocket PROMAX**: [https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.module](https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.module)

### 📱 手机轻量版（Lite）
- **Surge PROMAX Lite**: [https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/module/surge%28main%29/head_expanse/%F0%9F%93%B1%20Universal%20Ad-Blocking%20Rules%20%28PROMAX%20Lite%29.sgmodule](https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/module/surge%28main%29/head_expanse/%F0%9F%93%B1%20Universal%20Ad-Blocking%20Rules%20%28PROMAX%20Lite%29.sgmodule)
- **Shadowrocket PROMAX Lite**: [https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket/head_expanse/%F0%9F%93%B1%20Universal%20Ad-Blocking%20Rules%20%28PROMAX%20Lite%29.module](https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket/head_expanse/%F0%9F%93%B1%20Universal%20Ad-Blocking%20Rules%20%28PROMAX%20Lite%29.module)
