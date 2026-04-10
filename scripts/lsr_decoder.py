#!/usr/bin/env python3
"""
Loon .lsr Rule File Decoder
用法: python3 lsr_decoder.py <file.lsr> [output.list]

支持自动探测：gzip / zlib / 原始文本 / protobuf raw
"""

import sys
import zlib
import gzip
import struct
import io
import os

def try_gzip(data: bytes) -> bytes | None:
    try:
        return gzip.decompress(data)
    except Exception:
        return None

def try_zlib(data: bytes) -> bytes | None:
    try:
        return zlib.decompress(data)
    except Exception:
        pass
    # 跳过可能的自定义文件头，逐字节偏移尝试
    for offset in range(1, min(64, len(data))):
        try:
            return zlib.decompress(data[offset:])
        except Exception:
            pass
    return None

def try_zlib_raw(data: bytes) -> bytes | None:
    """wbits=-15 = raw deflate, no header"""
    try:
        return zlib.decompress(data, -15)
    except Exception:
        pass
    for offset in range(1, min(64, len(data))):
        try:
            return zlib.decompress(data[offset:], -15)
        except Exception:
            pass
    return None

def try_protobuf_raw(data: bytes) -> list[str] | None:
    """
    尝试以 protobuf raw decode 方式提取字符串字段
    无需 proto schema，直接读 wire type=2 (length-delimited) 字段
    """
    strings = []
    i = 0
    while i < len(data):
        try:
            # 读 tag varint
            tag_val = 0
            shift = 0
            while i < len(data):
                b = data[i]; i += 1
                tag_val |= (b & 0x7F) << shift
                shift += 7
                if not (b & 0x80):
                    break
            wire_type = tag_val & 0x07
            field_num = tag_val >> 3

            if wire_type == 0:  # varint
                while i < len(data) and (data[i] & 0x80):
                    i += 1
                i += 1
            elif wire_type == 1:  # 64-bit
                i += 8
            elif wire_type == 2:  # length-delimited
                length = 0; shift = 0
                while i < len(data):
                    b = data[i]; i += 1
                    length |= (b & 0x7F) << shift
                    shift += 7
                    if not (b & 0x80):
                        break
                val = data[i:i+length]
                i += length
                try:
                    s = val.decode('utf-8')
                    if any(kw in s for kw in ['DOMAIN', 'IP-CIDR', 'USER-AGENT', 'URL-REGEX', '.']):
                        strings.append(s)
                except Exception:
                    pass
            elif wire_type == 5:  # 32-bit
                i += 4
            else:
                break
        except Exception:
            break
    return strings if strings else None

def parse_rules_from_text(text: str) -> list[str]:
    """从文本中提取规则行"""
    rules = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith('#') or line.startswith('//'):
            continue
        # 常见规则前缀
        prefixes = ('DOMAIN', 'IP-CIDR', 'USER-AGENT', 'URL-REGEX',
                    'GEOIP', 'RULE-SET', 'PROCESS-NAME', 'DEST-PORT',
                    'AND', 'OR', 'NOT', 'FINAL')
        if any(line.startswith(p) for p in prefixes):
            rules.append(line)
        elif line.startswith('HOST') or ',' in line:
            rules.append(line)
    return rules

def decode_lsr_bytes(raw: bytes) -> tuple[list[str], str]:
    """
    Decodes LSR from raw bytes.
    Returns (rules_list, method_used)
    """
    # 1. 尝试直接作为文本
    try:
        text = raw.decode('utf-8')
        rules = parse_rules_from_text(text)
        if rules:
            return rules, "plaintext"
    except Exception:
        pass

    # 2. 尝试 gzip
    decompressed = try_gzip(raw)
    if decompressed:
        try:
            text = decompressed.decode('utf-8')
            rules = parse_rules_from_text(text)
            return rules, "gzip"
        except Exception:
            pass
        # 解压后再尝试 protobuf
        pb_strings = try_protobuf_raw(decompressed)
        if pb_strings:
            return pb_strings, "gzip+protobuf"

    # 3. 尝试 zlib (带头)
    decompressed = try_zlib(raw)
    if decompressed:
        try:
            text = decompressed.decode('utf-8')
            rules = parse_rules_from_text(text)
            return rules, "zlib"
        except Exception:
            pass
        pb_strings = try_protobuf_raw(decompressed)
        if pb_strings:
            return pb_strings, "zlib+protobuf"

    # 4. 尝试 raw deflate
    decompressed = try_zlib_raw(raw)
    if decompressed:
        try:
            text = decompressed.decode('utf-8')
            rules = parse_rules_from_text(text)
            return rules, "raw_deflate"
        except Exception:
            pass
        pb_strings = try_protobuf_raw(decompressed)
        if pb_strings:
            return pb_strings, "raw_deflate+protobuf"

    # 5. 直接尝试 protobuf raw 解析原始数据
    pb_strings = try_protobuf_raw(raw)
    if pb_strings:
        return pb_strings, "raw_protobuf"

    return [], "unknown"

def decode_lsr(filepath: str) -> tuple[list[str], str]:
    """
    主解码流程，返回 (rules_list, method_used)
    """
    with open(filepath, 'rb') as f:
        raw = f.read()

    print(f"[*] 文件大小: {len(raw)} bytes")
    print(f"[*] 文件头 (hex): {raw[:16].hex()}")
    print(f"[*] 文件头 (ascii): {raw[:16]}")

    # 1. 尝试直接作为文本
    try:
        text = raw.decode('utf-8')
        rules = parse_rules_from_text(text)
        if rules:
            print(f"[+] 方法: 纯文本，提取到 {len(rules)} 条规则")
            return rules, "plaintext"
    except Exception:
        pass

    # 2. 尝试 gzip
    decompressed = try_gzip(raw)
    if decompressed:
        print(f"[+] 方法: gzip 解压成功，解压后 {len(decompressed)} bytes")
        try:
            text = decompressed.decode('utf-8')
            rules = parse_rules_from_text(text)
            print(f"[+] 提取到 {len(rules)} 条规则")
            return rules, "gzip"
        except Exception:
            pass
        # 解压后再尝试 protobuf
        pb_strings = try_protobuf_raw(decompressed)
        if pb_strings:
            print(f"[+] gzip+protobuf，提取到 {len(pb_strings)} 条规则")
            return pb_strings, "gzip+protobuf"

    # 3. 尝试 zlib (带头)
    decompressed = try_zlib(raw)
    if decompressed:
        print(f"[+] 方法: zlib 解压成功，解压后 {len(decompressed)} bytes")
        try:
            text = decompressed.decode('utf-8')
            rules = parse_rules_from_text(text)
            print(f"[+] 提取到 {len(rules)} 条规则")
            return rules, "zlib"
        except Exception:
            pass
        pb_strings = try_protobuf_raw(decompressed)
        if pb_strings:
            print(f"[+] zlib+protobuf，提取到 {len(pb_strings)} 条规则")
            return pb_strings, "zlib+protobuf"

    # 4. 尝试 raw deflate
    decompressed = try_zlib_raw(raw)
    if decompressed:
        print(f"[+] 方法: raw deflate 解压成功，解压后 {len(decompressed)} bytes")
        try:
            text = decompressed.decode('utf-8')
            rules = parse_rules_from_text(text)
            print(f"[+] 提取到 {len(rules)} 条规则")
            return rules, "raw_deflate"
        except Exception:
            pass
        pb_strings = try_protobuf_raw(decompressed)
        if pb_strings:
            return pb_strings, "raw_deflate+protobuf"

    # 5. 直接尝试 protobuf raw 解析原始数据
    pb_strings = try_protobuf_raw(raw)
    if pb_strings:
        print(f"[+] 方法: raw protobuf，提取到 {len(pb_strings)} 条规则")
        return pb_strings, "raw_protobuf"

    print("[-] 所有方法均失败，请提供 hex dump 进一步分析")
    return [], "unknown"

def main():
    if len(sys.argv) < 2:
        print("用法: python3 lsr_decoder.py <file.lsr> [output.list]")
        sys.exit(1)

    filepath = sys.argv[1]
    output_path = sys.argv[2] if len(sys.argv) > 2 else filepath.replace('.lsr', '.list')

    rules, method = decode_lsr(filepath)

    if rules:
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(f"# Decoded from {os.path.basename(filepath)} via {method}\n")
            f.write(f"# Total rules: {len(rules)}\n\n")
            for r in rules:
                f.write(r + '\n')
        print(f"\n[+] 已保存 {len(rules)} 条规则到: {output_path}")
    else:
        # 至少输出 hex dump 供分析
        with open(filepath, 'rb') as f:
            data = f.read()
        print("\n[!] 无法解码，输出 hex dump 供人工分析：")
        for i in range(0, min(256, len(data)), 16):
            chunk = data[i:i+16]
            hex_part = ' '.join(f'{b:02x}' for b in chunk)
            asc_part = ''.join(chr(b) if 32 <= b < 127 else '.' for b in chunk)
            print(f"  {i:04x}  {hex_part:<47}  {asc_part}")

if __name__ == '__main__':
    main()
