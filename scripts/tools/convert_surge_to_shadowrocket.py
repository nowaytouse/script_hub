#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge → Shadowrocket Config & Module Converter
Converts Surge configurations and modules to Shadowrocket format with robust error handling
"""

import re
import sys
import urllib.request
import urllib.error
from pathlib import Path
from typing import Optional, List, Dict

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS_DIR))
PROJECT_ROOT = SCRIPTS_DIR.parent

from hub.module_sanitizer import sanitize_file_content, parse_module, format_module, format_header
from hub.common import _BROWSER_UA, Logger, write_file
from hub.sr_module_adapter import (
    DEVTOOLS_STEM,
    adapt_mitm_line_for_sr,
    adapt_script_line_for_sr,
    module_stem_from_meta,
)
SURGE_MODULE_DIR = PROJECT_ROOT / "modules" / "surge"
SR_MODULE_DIR = PROJECT_ROOT / "modules" / "shadowrocket"

# 配置常量
DOWNLOAD_TIMEOUT = 10  # 秒
MAX_RETRIES = 3
CACHE_SIZE_LIMIT = 100  # 最大缓存规则集数量

# 缓存已下载的规则集
RULESET_CACHE = {}

SURGE_ONLY_KEYS = {
    "use-local-host-item-for-proxy", "encrypted-dns-follow-outbound-mode",
    "encrypted-dns-skip-cert-verification", "force-http-engine-hosts",
    "always-raw-tcp-hosts", "always-raw-tcp-keywords", "tun-included-routes",
    "compatibility-mode", "http-api-tls", "http-api-web-dashboard",
    "allow-wifi-access", "allow-hotspot-access", "wifi-access-http-port",
    "wifi-access-socks5-port", "proxy-restricted-to-lan", "block-quic",
    "exclude-simple-hostnames", "read-etc-hosts", "include-apns",
    "include-cellular-services", "wifi-assist", "allow-dns-svcb",
    "ipv6-vif", "include-all-networks", "include-local-networks",
    "auto-suspend", "all-hybrid", "http-listen", "socks5-listen",
    "proxy-test-udp",
}

GENERAL_KEY_MAP = {
    "encrypted-dns-server": "dns-server",  # DoH 首选 - 使用 dns-server 字段
    "dns-server": "fallback-dns-server",   # 明文备用
    "tun-excluded-routes": "tun-excluded-routes",
}

RULE_REPLACEMENTS = {
    "REJECT-DROP": "REJECT", "REJECT-TINYGIF": "REJECT", "REJECT-NO-DROP": "REJECT",
}

# Keep remote RULE-SET references for purpose-split AdBlock shards
PRESERVE_RULESET_MARKERS = (
    "/rulesets/AdBlock/AdBlock_",
    "ruleset%2FAdBlock%2FAdBlock_",
    "skk_upstream/",
    "HTTPDNS_Hijack.list",
    "nowaytouse/script_hub@master/rulesets/",
)

REWRITE_MODIFIER_RE = re.compile(r',\s*(extended-matching|pre-matching)\b|'
                                  r'\b(extended-matching|pre-matching)\s*,?')

# Injected once per [General] when Surge module has no General section defaults
SR_GENERAL_DEFAULTS = """bypass-system = true
ipv6 = true
prefer-ipv6 = true
hijack-dns = *:53
dns-direct-fallback-proxy = false"""

# Host 部分需要保留的关键域名模式
HOST_KEEP_PATTERNS = [
    r'^dns\.',           # DNS 提供商
    r'^doh\.',           # DoH 提供商
    r'^cloudflare',      # Cloudflare
    r'\.google\.com$',   # Google 服务
    r'^talk\.google',    # FCM
    r'^mtalk\.google',   # FCM
    r'^alt\d+-mtalk',    # FCM
    r'^stun',            # STUN
    r'^connectivitycheck', # 连通性检查
    r'^detectportal',    # Portal 检测
    r'^msftconnecttest', # Microsoft 连通性
    r'^msftncsi',        # Microsoft NCSI
    r'^\*\.cn$',         # 大陆域名
    r'^\*\.tw$',         # 台湾域名
    r'^\*\.cht\.com\.tw$', # 台湾运营商
    r'^\*\.hinet\.net$', # 台湾运营商
    r'^\*\.he\.net$',    # HE.net
    r'^hanime1\.me$',    # NSFW
    r'^18comic\.vip$',   # NSFW
    r'^3hentai\.net$',   # NSFW
    r'^router\.',        # 路由器
    r'^miwifi\.',        # 路由器
    r'^tplogin\.',       # 路由器
    r'\.liangxin1\.xyz$', # 代理基础设施
    r'^raw\.githubusercontent', # GitHub
    r'^github\.com$',    # GitHub
    r'^\d+\.\d+\.\d+\.\d+$', # IP 地址
]

def should_keep_host_line(line: str) -> bool:
    """判断 Host 行是否应该保留"""
    stripped = line.strip()
    if not stripped or stripped.startswith('#'):
        return True

    # 提取域名部分
    if '=' in stripped:
        domain = stripped.split('=')[0].strip()
        for pattern in HOST_KEEP_PATTERNS:
            if re.search(pattern, domain, re.IGNORECASE):
                return True
    return False

def fetch_ruleset(url_or_path: str) -> List[str]:
    """抓取并解析规则集，带重试和错误处理

    Args:
        url_or_path: 规则集 URL 或本地路径

    Returns:
        解析后的规则列表，失败返回空列表
    """
    if url_or_path in RULESET_CACHE:
        return RULESET_CACHE[url_or_path]

    # 限制缓存大小
    if len(RULESET_CACHE) >= CACHE_SIZE_LIMIT:
        Logger.warn(f"规则集缓存已满 ({CACHE_SIZE_LIMIT})，清空缓存")
        RULESET_CACHE.clear()

    content = ""

    try:
        # 尝试本地文件（jsdelivr CDN 映射）
        cdn_patterns = [
            "fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/",
            "cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/",
            "gcore.jsdelivr.net/gh/nowaytouse/script_hub@master/"
        ]

        for cdn in cdn_patterns:
            if cdn in url_or_path:
                rel_path = url_or_path.split("@master/")[-1]
                local_path = PROJECT_ROOT / rel_path
                if local_path.exists():
                    Logger.info(f"使用本地文件: {local_path.name}")
                    try:
                        content = local_path.read_text(encoding='utf-8')
                        break
                    except (IOError, UnicodeDecodeError) as e:
                        Logger.warn(f"读取本地文件失败: {e}")
                        continue

        # 相对路径处理
        if not content and url_or_path.startswith(".."):
            path = (PROJECT_ROOT / "modules/surge/amplify_nexus" / url_or_path).resolve()
            if path.exists():
                try:
                    content = path.read_text(encoding='utf-8')
                except (IOError, UnicodeDecodeError) as e:
                    Logger.error(f"读取相对路径文件失败 {path}: {e}")
                    return []

        # 远程 URL 下载（带重试）
        if not content and url_or_path.startswith("http"):
            for attempt in range(MAX_RETRIES):
                try:
                    Logger.info(f"下载规则集 (尝试 {attempt + 1}/{MAX_RETRIES}): {url_or_path}")
                    req = urllib.request.Request(url_or_path, headers={'User-Agent': _BROWSER_UA})
                    with urllib.request.urlopen(req, timeout=DOWNLOAD_TIMEOUT) as response:
                        content = response.read().decode('utf-8')
                    break
                except urllib.error.URLError as e:
                    Logger.warn(f"下载失败 (尝试 {attempt + 1}): {e}")
                    if attempt == MAX_RETRIES - 1:
                        Logger.error(f"下载规则集失败，已重试 {MAX_RETRIES} 次: {url_or_path}")
                        return []
                except Exception as e:
                    Logger.error(f"下载规则集时发生未知错误: {e}")
                    return []

        if not content:
            Logger.warn(f"无法获取规则集内容: {url_or_path}")
            return []

        # 解析规则
        rules = []
        for line in content.splitlines():
            line = line.strip()
            if not line or line.startswith(("#", "//", ";")):
                continue

            parts = line.split(',')
            if not parts:
                continue

            rtype = parts[0].upper()
            if rtype in ("DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "IP-CIDR", "IP-CIDR6", "GEOIP"):
                if len(parts) >= 2:
                    rules.append(",".join(parts[:2]))

        RULESET_CACHE[url_or_path] = rules
        Logger.success(f"成功解析规则集: {len(rules)} 条规则")
        return rules

    except Exception as e:
        Logger.error(f"解析规则集时发生错误 {url_or_path}: {e}")
        return []

def convert_content(content: str, *, module_stem: str = "") -> str:
    if not module_stem:
        meta, _ = parse_module(content)
        module_stem = module_stem_from_meta(meta)

    lines = content.split('\n')
    out = []
    section = None
    general_defaults_added = False

    for line in lines:
        stripped = line.strip()

        if stripped.startswith('[') and stripped.endswith(']'):
            section = stripped[1:-1]
            out.append(line)
            if section == "General" and not general_defaults_added:
                out.append(SR_GENERAL_DEFAULTS)
                general_defaults_added = True
            continue

        if re.match(r'^#!', line):
            if re.match(r'^#!\s*(update-interval|ability)\s*=', line, re.IGNORECASE):
                continue
            m = re.match(r'^#!\s*(\S+?)\s*=\s*(.*)$', line)
            if m:
                key, val = m.group(1).strip(), m.group(2)
                if key == "desc" and "[🚀SR]" not in val:
                    val = f"[🚀SR] {val}"
                # Preserve arguments / arguments-desc verbatim for module settings UI
                out.append(f"#!{key}={val}")
            else:
                out.append(line)
            continue

        if section == "General" and not stripped.startswith('#') and stripped:
            if any(k in line for k in SURGE_ONLY_KEYS):
                out.append(f"# [SR不支持] {line.lstrip()}")
                continue
            for k, v in GENERAL_KEY_MAP.items():
                if line.startswith(k):
                    line = line.replace(k, v)
            out.append(line)
            continue

        if section == "Rule" and not stripped.startswith('#') and stripped:
            if stripped.startswith('RULE-SET,'):
                parts = stripped.split(',')
                url_or_path = parts[1].strip()
                policy = parts[2].strip() if len(parts) > 2 else "REJECT"
                policy = RULE_REPLACEMENTS.get(policy, policy)

                if any(marker in url_or_path for marker in PRESERVE_RULESET_MARKERS):
                    cleaned = REWRITE_MODIFIER_RE.sub('', line)
                    cleaned = re.sub(r',update-interval=\d+', '', cleaned)
                    cleaned = re.sub(r',no-resolve', '', cleaned)
                    for old, new in RULE_REPLACEMENTS.items():
                        cleaned = cleaned.replace(old, new)
                    out.append(cleaned.strip())
                    continue

                print(f"  📦 Expanding RULE-SET: {url_or_path}")
                expanded_rules = fetch_ruleset(url_or_path)
                if expanded_rules:
                    out.append(f"# --- Expanded from {url_or_path} ---")
                    for r in expanded_rules:
                        out.append(f"{r},{policy}")
                    out.append(f"# --- End expansion ---")
                else:
                    out.append(f"# [无法展开] {line}")
                continue
            
            if stripped.startswith('PROTOCOL,'):
                out.append(f"# [SR不支持PROTOCOL] {line}")
                continue

        if section == "Script" and stripped and not stripped.startswith('#'):
            line = adapt_script_line_for_sr(line, module_stem=module_stem)
        elif section == "MITM" and stripped and not stripped.startswith('#'):
            line = adapt_mitm_line_for_sr(line)

        line = re.sub(r'%(?:INSERT|APPEND)%\s*', '', line)
        for old, new in RULE_REPLACEMENTS.items():
            line = line.replace(old, new)
        line = REWRITE_MODIFIER_RE.sub('', line)
        line = re.sub(r',"update-interval=\d+"', '', line)
        
        out.append(line)

    return sanitize_file_content('\n'.join(out), dedupe=True)


def convert_devtools_module_for_sr(content: str) -> str:
    """Shadowrocket build for devtools bundle: SR script compat + note in header."""
    meta, sections = parse_module(content)
    module_stem = module_stem_from_meta(meta, DEVTOOLS_STEM)
    converted = convert_content(content, module_stem=module_stem)
    meta, sections = parse_module(converted)
    desc = meta.get("desc", "")
    if "[🚀SR]" not in desc:
        meta["desc"] = (
            "[🚀SR] " + desc
            + "\\nSub-Store 已按 Surge-Noability 精简(无 ability)；produce 定时仅 Surge 版可用"
        )
    section_map = {name: lines for name, lines in sections}
    header = format_header(meta)
    ordered = [(n, section_map[n]) for n in section_map]
    return sanitize_file_content(format_module(header, ordered, dedupe=True), dedupe=True)

PROMAX_SURGE_MODULES = (
    "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
    "📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule",
)


def convert_promax_modules() -> bool:
    """Convert only Surge PROMAX / PROMAX Lite → Shadowrocket head_expanse."""
    cat = "head_expanse"
    cat_path = SURGE_MODULE_DIR / cat
    out_dir = SR_MODULE_DIR / cat
    out_dir.mkdir(parents=True, exist_ok=True)
    ok = True
    for name in PROMAX_SURGE_MODULES:
        src = cat_path / name
        if not src.is_file():
            Logger.warn(f"PROMAX source missing: {name}")
            ok = False
            continue
        Logger.info(f"Converting PROMAX → SR: {name}")
        try:
            converted = convert_content(src.read_text(encoding="utf-8"))
            out_path = out_dir / (src.stem + ".module")
            write_file(str(out_path), converted)
            Logger.success(f"SR module written: {out_path.name}")
        except Exception as exc:
            Logger.error(f"PROMAX SR conversion failed [{name}]: {exc}")
            ok = False
    return ok


def process_all_modules():
    if not SR_MODULE_DIR.exists():
        SR_MODULE_DIR.mkdir(parents=True)

    categories = ["amplify_nexus", "head_expanse", "narrow_pierce"]
    stats = {"total": 0, "converted": 0, "failed": 0}
    
    for cat in categories:
        (SR_MODULE_DIR / cat).mkdir(exist_ok=True)
        cat_path = SURGE_MODULE_DIR / cat
        if not cat_path.exists(): continue

        for module_file in sorted(cat_path.glob("*.sgmodule")):
            stats["total"] += 1
            print(f"🔄 Converting: {module_file.name}")
            try:
                content = module_file.read_text(encoding="utf-8")
                if module_file.stem == DEVTOOLS_STEM:
                    converted = convert_devtools_module_for_sr(content)
                else:
                    converted = convert_content(content, module_stem=module_file.stem)
                out_path = SR_MODULE_DIR / cat / (module_file.stem + ".module")
                write_file(str(out_path), converted)
                stats["converted"] += 1
            except Exception as e:
                print(f"  ❌ Failed {module_file.name}: {e}")
                stats["failed"] += 1

    return stats

def convert_proxy_group_line(line: str) -> str:
    """转换 Surge 代理组语法到 Shadowrocket"""
    # 移除 Shadowrocket 不支持的参数
    line = re.sub(r',\s*no-alert=\d+', '', line)
    line = re.sub(r',\s*hidden=\d+', '', line)
    line = re.sub(r',\s*persistent=\d+', '', line)
    line = re.sub(r',\s*evaluate-before-use=\d+', '', line)
    line = re.sub(r',\s*include-all-proxies=\d+', '', line)
    line = re.sub(r',\s*icon-url=[^\s,]+', '', line)

    # 转换 smart 为 url-test
    line = re.sub(r'=\s*smart,', '= url-test,', line)

    # 转换 include-other-group 为 policy-path (如果没有 policy-path)
    if 'include-other-group=' in line and 'policy-path=' not in line:
        # 提取第一个 include-other-group 的值作为 policy-path
        match = re.search(r'include-other-group="([^"]+)"', line)
        if match:
            first_group = match.group(1).split('", "')[0]
            line = re.sub(r'include-other-group="[^"]+"', '', line)

    return line

def convert_main_config(surge_conf_path: Path, output_path: Path, compact_host: bool = True) -> bool:
    """转换完整的 Surge 主配置文件到 Shadowrocket

    Args:
        surge_conf_path: Surge 配置文件路径
        output_path: 输出路径
        compact_host: 是否精简 Host 部分（默认 True，只保留关键域名）

    Returns:
        转换是否成功
    """
    Logger.section(f"转换主配置: {surge_conf_path.name}")

    # 验证输入文件
    if not surge_conf_path.exists():
        Logger.error(f"配置文件不存在: {surge_conf_path}")
        return False

    if not surge_conf_path.is_file():
        Logger.error(f"路径不是文件: {surge_conf_path}")
        return False

    try:
        content = surge_conf_path.read_text(encoding='utf-8')
    except (IOError, UnicodeDecodeError) as e:
        Logger.error(f"读取配置文件失败: {e}")
        return False

    lines = content.split('\n')
    out = []
    section = None
    host_lines_kept = 0
    host_lines_skipped = 0

    # 缓存 DNS 配置，确保正确顺序
    dns_server_line = None  # 首选 DNS（DoH）
    fallback_dns_line = None  # 备用 DNS（明文）

    try:
        for line in lines:
            stripped = line.strip()

            # 检测段落
            if stripped.startswith('[') and stripped.endswith(']'):
                # 在离开 General 段落前，输出缓存的 DNS 配置（正确顺序）
                if section == "General" and (dns_server_line or fallback_dns_line):
                    if dns_server_line:
                        out.append(dns_server_line)
                    if fallback_dns_line:
                        out.append(fallback_dns_line)
                    # 重置缓存
                    dns_server_line = None
                    fallback_dns_line = None

                section = stripped[1:-1]
                out.append(line)

                # Host 段落添加说明
                if section == "Host" and compact_host:
                    out.append("# Shadowrocket 精简版 - 仅保留关键 DNS 配置")
                    out.append("# 完整域名级 DoH 分流通过 [General] doh-server 全局配置实现")
                continue

            # 跳过 [Ponte] 段落（Surge 专属）
            if section == "Ponte":
                if not stripped.startswith('#') and stripped:
                    out.append(f"# [SR不支持Ponte] {line}")
                else:
                    out.append(line)
                continue

            # 处理 [General]
            if section == "General" and not stripped.startswith('#') and stripped:
                if any(k in line for k in SURGE_ONLY_KEYS):
                    out.append(f"# [SR不支持] {line.lstrip()}")
                    continue

                # 转换字段名并缓存 DNS 配置
                converted = False
                for k, v in GENERAL_KEY_MAP.items():
                    if line.startswith(k):
                        line = line.replace(k, v, 1)
                        converted = True

                        # 缓存 DNS 配置，稍后按正确顺序输出
                        if k == "encrypted-dns-server":
                            dns_server_line = line  # DoH 首选
                            break
                        elif k == "dns-server":
                            fallback_dns_line = line  # 明文备用
                            break

                # 非 DNS 配置直接输出
                if not converted or (k not in ("encrypted-dns-server", "dns-server")):
                    out.append(line)
                continue

            # 处理 [Proxy Group]
            if section == "Proxy Group" and not stripped.startswith('#') and stripped and '=' in stripped:
                line = convert_proxy_group_line(line)
                out.append(line)
                continue

            # 处理 [Rule]
            if section == "Rule" and not stripped.startswith('#') and stripped:
                # 移除 extended-matching
                line = re.sub(r',\s*extended-matching', '', line)
                # 移除 no-resolve 后的逗号
                line = re.sub(r',\s*no-resolve\s*$', '', line)
                # 移除 PROTOCOL 规则
                if stripped.startswith('PROTOCOL,'):
                    out.append(f"# [SR不支持PROTOCOL] {line}")
                    continue
                out.append(line)
                continue

            # 处理 [Host] - 精简模式
            if section == "Host":
                if compact_host:
                    if should_keep_host_line(line):
                        # 转换 server:force-syslib 为 server:syslib
                        line = re.sub(r'server:force-syslib', 'server:syslib', line)
                        out.append(line)
                        if not stripped.startswith('#') and stripped:
                            host_lines_kept += 1
                    else:
                        host_lines_skipped += 1
                else:
                    # 完整模式：保留所有 Host 配置
                    line = re.sub(r'server:force-syslib', 'server:syslib', line)
                    out.append(line)
                continue

            # 处理 [Script] - Shadowrocket 不支持，注释掉
            if section == "Script":
                if not stripped.startswith('#') and stripped:
                    out.append(f"# [SR不支持Script] {line}")
                else:
                    out.append(line)
                continue

            # 其他行直接输出
            out.append(line)

        result = '\n'.join(out)

        # 使用原子写入
        try:
            write_file(str(output_path), result)
            Logger.success(f"配置已转换: {output_path}")
            if compact_host:
                Logger.info(f"Host 精简: 保留 {host_lines_kept} 行，跳过 {host_lines_skipped} 行")
            Logger.info(f"总行数: {len(out)}")
            return True

        except Exception as e:
            Logger.error(f"写入配置文件失败: {e}")
            return False

    except Exception as e:
        Logger.error(f"转换配置时发生错误: {e}")
        return False


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(
        description="Convert Surge configs/modules to Shadowrocket",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # 转换主配置（默认精简 Host）
  python3 convert_surge_to_shadowrocket.py

  # 转换指定配置文件
  python3 convert_surge_to_shadowrocket.py --config path/to/surge.conf

  # 转换配置并保留完整 Host 部分
  python3 convert_surge_to_shadowrocket.py --config surge.conf --full-host

  # 转换所有模块
  python3 convert_surge_to_shadowrocket.py --modules
        """
    )
    parser.add_argument("--modules", action="store_true", help="Convert all Surge modules to Shadowrocket")
    parser.add_argument(
        "--promax-only",
        action="store_true",
        help="Convert only PROMAX / PROMAX Lite (after adblock_manager merge)",
    )
    parser.add_argument("--config", type=Path, help="Convert a specific Surge config file")
    parser.add_argument("--output", type=Path, help="Output path for config conversion")
    parser.add_argument("--full-host", action="store_true", help="Keep full Host section (default: compact)")

    args = parser.parse_args()

    exit_code = 0

    try:
        if args.config:
            # 转换指定的主配置文件
            if not args.config.exists():
                Logger.error(f"配置文件不存在: {args.config}")
                sys.exit(1)

            output = args.output or args.config.parent / (args.config.stem + "_Shadowrocket.conf")
            success = convert_main_config(args.config, output, compact_host=not args.full_host)
            exit_code = 0 if success else 1

        elif args.promax_only:
            exit_code = 0 if convert_promax_modules() else 1

        elif args.modules:
            try:
                s = process_all_modules()
                Logger.success(f"所有模块已转换: {s['converted']}/{s['total']}")
                if s["failed"] > 0:
                    Logger.warn(f"失败: {s['failed']} 个模块")
                exit_code = 1 if s["converted"] == 0 else 0
            except Exception as e:
                Logger.error(f"转换模块时发生错误: {e}")
                exit_code = 1

        else:
            # 默认：本地有 .claude 配置则转换；CI/无配置时仅转换模块（.claude 在 .gitignore）
            surge_conf = PROJECT_ROOT / ".claude" / "NyaMiiKo.conf.conf"
            if surge_conf.exists():
                output = PROJECT_ROOT / ".claude" / "NyaMiiKo_Shadowrocket.conf"
                Logger.section("转换项目默认配置")
                success = convert_main_config(surge_conf, output, compact_host=not args.full_host)
                exit_code = 0 if success else 1
            else:
                Logger.warn("未找到 .claude/NyaMiiKo.conf.conf，跳过主配置，仅转换模块")
                s = process_all_modules()
                Logger.success(f"所有模块已转换: {s['converted']}/{s['total']}")
                if s["failed"] > 0:
                    Logger.warn(f"失败: {s['failed']} 个模块")
                exit_code = 1 if s["converted"] == 0 else 0

    except KeyboardInterrupt:
        Logger.warn("\n用户中断操作")
        sys.exit(130)
    except Exception as e:
        Logger.error(f"发生未预期的错误: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

    sys.exit(exit_code)
