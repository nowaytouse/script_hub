#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
统一的规则处理器 - 消除 ruleset_manager 和 adblock_manager 之间的重复代码
提供规则标准化、去重、验证的单一来源
"""

import re
from typing import Optional, Set, Dict, List, Tuple
from pathlib import Path

# 规则类型正则
DOMAIN_RULE = re.compile(r'^DOMAIN,([^,]+)$', re.IGNORECASE)
DOMAIN_SUFFIX_RULE = re.compile(r'^DOMAIN-SUFFIX,([^,]+)$', re.IGNORECASE)
DOMAIN_KEYWORD_RULE = re.compile(r'^DOMAIN-KEYWORD,([^,]+)$', re.IGNORECASE)
IP_CIDR_RULE = re.compile(r'^IP-CIDR6?,([^,]+)$', re.IGNORECASE)
DOMAIN_REGEX_RULE = re.compile(r'^DOMAIN-REGEX,(.+)$', re.IGNORECASE)

# 域名验证（宽松模式，允许通配符）
VALID_DOMAIN = re.compile(
    r'^(\*\.)?[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?'
    r'(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$'
)

# IP-CIDR 验证
VALID_IPV4_CIDR = re.compile(r'^(\d{1,3}\.){3}\d{1,3}(/\d{1,2})?$')
VALID_IPV6_CIDR = re.compile(r'^([0-9a-fA-F:]+)(/\d{1,3})?$')

# Surge rejects these DOMAIN-REGEX payloads; MetaCubeX deco often emits char-split junk.
INVALID_DOMAIN_REGEX_VALUES = frozenset({"", "$", ",", "-", ".", "2", "6", "]", "["})

# Truncated URL-REGEX values produced when `//` in https:// was stripped as a comment.
INVALID_URL_REGEX_VALUES = frozenset({
    "", "https:", "http:", "https", "http",
    "^https?:", "^https?://", "^https?:\\/\\/",
})

PAYLOAD_SAFE_RULE_TYPES = frozenset({
    "DOMAIN-REGEX", "URL-REGEX", "USER-AGENT", "PROCESS-NAME",
})


class RuleProcessor:
    """统一的规则处理器"""

    def __init__(self, strict_validation: bool = False):
        """
        Args:
            strict_validation: 是否启用严格验证（拒绝可疑规则）
        """
        self.strict_validation = strict_validation
        self.stats = {
            'total': 0,
            'normalized': 0,
            'invalid': 0,
            'duplicate': 0,
        }

    def normalize_rule(self, line: str, source: str = "") -> Optional[str]:
        """标准化单条规则

        Args:
            line: 原始规则行
            source: 规则来源（用于日志）

        Returns:
            标准化后的规则，无效返回 None
        """
        self.stats['total'] += 1

        # 清理空白和注释
        line = line.strip()
        if not line or line.startswith(('#', ';')):
            return None
        if line.startswith('//'):
            return None

        rule_head = line.split(',', 1)[0].strip().upper() if ',' in line else ''
        payload_safe = rule_head in PAYLOAD_SAFE_RULE_TYPES

        # Do not treat `https://` inside URL-REGEX as an end-of-line comment.
        if not payload_safe:
            if '//' in line:
                line = line.split('//')[0].strip()
            if '#' in line:
                line = line.split('#')[0].strip()

        if ',' not in line:
            self.stats['invalid'] += 1
            return None

        rule_type, rule_value = line.split(',', 1)
        rule_type = rule_type.strip().upper()
        rule_value = rule_value.strip()

        if rule_type.startswith("IP-"):
            # Sources often append ,no-resolve; keep CIDR only for validation/output.
            rule_value = rule_value.split(",", 1)[0].strip()

        # 验证规则类型
        if rule_type not in ('DOMAIN', 'DOMAIN-SUFFIX', 'DOMAIN-KEYWORD',
                              'IP-CIDR', 'IP-CIDR6', 'DOMAIN-REGEX', 'GEOIP',
                              'USER-AGENT', 'URL-REGEX', 'PROCESS-NAME'):
            self.stats['invalid'] += 1
            return None

        if rule_value.startswith('"') and rule_value.endswith('"'):
            rule_value = rule_value[1:-1]

        # 验证规则值
        if not self._validate_rule_value(rule_type, rule_value):
            self.stats['invalid'] += 1
            return None

        normalized = self._format_surge_rule(rule_type, rule_value)
        self.stats['normalized'] += 1
        return normalized

    @staticmethod
    def _format_surge_rule(rule_type: str, rule_value: str) -> str:
        """Surge .list / RULE-SET: quote regex payloads (required for | ( ) $ , etc.)."""
        if rule_type == "DOMAIN-REGEX":
            return f'DOMAIN-REGEX,"{rule_value}"'
        if rule_type == "URL-REGEX" and ("," in rule_value or " " in rule_value):
            return f'URL-REGEX,"{rule_value}"'
        return f"{rule_type},{rule_value}"

    def _validate_rule_value(self, rule_type: str, value: str) -> bool:
        """验证规则值的有效性"""
        if not value:
            return False

        # 域名规则验证
        if rule_type in ('DOMAIN', 'DOMAIN-SUFFIX'):
            # 移除前导点
            value = value.lstrip('.')
            if self.strict_validation:
                return bool(VALID_DOMAIN.match(value))
            # 宽松模式：只检查基本格式
            return '.' in value or len(value) > 2

        # 关键词规则（宽松）
        if rule_type == 'DOMAIN-KEYWORD':
            return len(value) >= 2 and not any(c in value for c in '<>{}()')

        # IP-CIDR 验证
        if rule_type == 'IP-CIDR':
            return bool(VALID_IPV4_CIDR.match(value))
        if rule_type == 'IP-CIDR6':
            return bool(VALID_IPV6_CIDR.match(value))

        # DOMAIN-REGEX（过滤 MetaCubeX 拆散的无效片段）
        if rule_type == 'DOMAIN-REGEX':
            if len(value.strip()) < 2 or value in INVALID_DOMAIN_REGEX_VALUES:
                return False
            try:
                re.compile(value)
                return True
            except re.error:
                return False

        # URL-REGEX（只丢弃明显无效项，保留合法规则）
        if rule_type == 'URL-REGEX':
            val_lower = value.lower().strip()
            if val_lower in INVALID_URL_REGEX_VALUES or len(value.strip()) < 3:
                return False
            try:
                re.compile(value)
                return True
            except re.error:
                return False

        # 其他类型（宽松）
        return True

    def deduplicate_rules(self, rules: List[str]) -> List[str]:
        """去重规则列表

        Args:
            rules: 规则列表

        Returns:
            去重后的规则列表（保持顺序）
        """
        seen = set()
        result = []
        for rule in rules:
            if rule not in seen:
                seen.add(rule)
                result.append(rule)
            else:
                self.stats['duplicate'] += 1
        return result

    def deduplicate_by_priority(self, rules_dict: Dict[str, Set[str]]) -> Dict[str, Set[str]]:
        """按优先级去重规则集

        当同一规则出现在多个规则集时，保留在优先级最高的规则集中

        Args:
            rules_dict: {ruleset_name: {rules}}

        Returns:
            去重后的规则字典
        """
        # 构建全局规则 -> 规则集映射
        rule_to_sources: Dict[str, List[str]] = {}
        for source, rules in rules_dict.items():
            for rule in rules:
                if rule not in rule_to_sources:
                    rule_to_sources[rule] = []
                rule_to_sources[rule].append(source)

        # 去重：每条规则只保留在第一个出现的规则集中
        result = {source: set() for source in rules_dict.keys()}
        for rule, sources in rule_to_sources.items():
            if len(sources) == 1:
                result[sources[0]].add(rule)
            else:
                # 多个来源，保留在第一个（按字典序）
                primary = sorted(sources)[0]
                result[primary].add(rule)
                self.stats['duplicate'] += len(sources) - 1

        return result

    def extract_domains_from_rules(self, rules: List[str]) -> Set[str]:
        """从规则中提取域名

        Args:
            rules: 规则列表

        Returns:
            域名集合
        """
        domains = set()
        for rule in rules:
            # DOMAIN,example.com
            m = DOMAIN_RULE.match(rule)
            if m:
                domains.add(m.group(1))
                continue

            # DOMAIN-SUFFIX,example.com
            m = DOMAIN_SUFFIX_RULE.match(rule)
            if m:
                domains.add(m.group(1))
                continue

        return domains

    def filter_by_whitelist(self, rules: List[str], whitelist: Set[str]) -> List[str]:
        """过滤白名单中的规则

        Args:
            rules: 规则列表
            whitelist: 白名单域名集合

        Returns:
            过滤后的规则列表
        """
        result = []
        for rule in rules:
            # 提取域名
            m = DOMAIN_RULE.match(rule) or DOMAIN_SUFFIX_RULE.match(rule)
            if m:
                domain = m.group(1).lower()
                # 检查是否在白名单中
                if domain in whitelist:
                    continue
                # 检查是否是白名单域名的子域名
                if any(domain.endswith('.' + w) for w in whitelist):
                    continue
            result.append(rule)
        return result

    def split_by_type(self, rules: List[str]) -> Dict[str, List[str]]:
        """按规则类型分类

        Args:
            rules: 规则列表

        Returns:
            {rule_type: [rules]}
        """
        result: Dict[str, List[str]] = {}
        for rule in rules:
            parts = rule.split(',', 1)
            if len(parts) >= 2:
                rule_type = parts[0].upper()
                if rule_type not in result:
                    result[rule_type] = []
                result[rule_type].append(rule)
        return result

    def get_stats(self) -> Dict[str, int]:
        """获取处理统计"""
        return self.stats.copy()

    def reset_stats(self):
        """重置统计"""
        self.stats = {
            'total': 0,
            'normalized': 0,
            'invalid': 0,
            'duplicate': 0,
        }


# 全局单例（可选）
_default_processor = RuleProcessor()


def normalize_rule(line: str, source: str = "") -> Optional[str]:
    """便捷函数：使用默认处理器标准化规则"""
    return _default_processor.normalize_rule(line, source)


def deduplicate_rules(rules: List[str]) -> List[str]:
    """便捷函数：使用默认处理器去重"""
    return _default_processor.deduplicate_rules(rules)
