#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
统一的规则处理器 - 消除 ruleset_manager 和 adblock_manager 之间的重复代码
提供规则标准化、去重、验证的单一来源
"""

import re
from typing import Optional, Set, Dict, List

from hub.surge_compliance import (
    INVALID_DOMAIN_REGEX_VALUES,
    INVALID_URL_REGEX_VALUES,
    is_invalid_domain_regex_payload,
    is_invalid_url_regex_payload,
    strip_inline_comment,
    format_url_regex_for_surge,
)

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

        line = strip_inline_comment(line.strip())
        if not line or line.startswith(('#', ';')):
            return None

        if ',' not in line:
            self.stats['invalid'] += 1
            return None

        rule_type, rule_value = line.split(',', 1)
        rule_type = rule_type.strip().upper()
        rule_value = rule_value.strip()

        if rule_type.startswith("IP-"):
            rule_value = rule_value.split(",", 1)[0].strip()

        if rule_type not in ('DOMAIN', 'DOMAIN-SUFFIX', 'DOMAIN-KEYWORD',
                              'IP-CIDR', 'IP-CIDR6', 'DOMAIN-REGEX', 'GEOIP',
                              'USER-AGENT', 'URL-REGEX', 'PROCESS-NAME'):
            self.stats['invalid'] += 1
            return None

        if rule_value.startswith('"') and rule_value.endswith('"'):
            rule_value = rule_value[1:-1]

        if not self._validate_rule_value(rule_type, rule_value):
            self.stats['invalid'] += 1
            return None

        normalized = self._format_rule(rule_type, rule_value)
        self.stats['normalized'] += 1
        return normalized

    @staticmethod
    def _format_rule(rule_type: str, rule_value: str) -> str:
        if rule_type == "URL-REGEX":
            return format_url_regex_for_surge(rule_value)
        return f"{rule_type},{rule_value}"

    def _validate_rule_value(self, rule_type: str, value: str) -> bool:
        """验证规则值的有效性"""
        if not value:
            return False

        if rule_type in ('DOMAIN', 'DOMAIN-SUFFIX'):
            value = value.lstrip('.')
            if self.strict_validation:
                return bool(VALID_DOMAIN.match(value))
            return '.' in value or len(value) > 2

        if rule_type == 'DOMAIN-KEYWORD':
            return len(value) >= 2 and not any(c in value for c in '<>{}()')

        if rule_type == 'IP-CIDR':
            return bool(VALID_IPV4_CIDR.match(value))
        if rule_type == 'IP-CIDR6':
            return bool(VALID_IPV6_CIDR.match(value))

        if rule_type == 'DOMAIN-REGEX':
            if is_invalid_domain_regex_payload(value):
                return False
            try:
                re.compile(value)
                return True
            except re.error:
                return False

        if rule_type == 'URL-REGEX':
            if is_invalid_url_regex_payload(value):
                return False
            try:
                re.compile(value)
                return True
            except re.error:
                return False

        return True

    def deduplicate_rules(self, rules: List[str]) -> List[str]:
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
        rule_to_sources: Dict[str, List[str]] = {}
        for source, rules in rules_dict.items():
            for rule in rules:
                if rule not in rule_to_sources:
                    rule_to_sources[rule] = []
                rule_to_sources[rule].append(source)

        result = {source: set() for source in rules_dict.keys()}
        for rule, sources in rule_to_sources.items():
            if len(sources) == 1:
                result[sources[0]].add(rule)
            else:
                primary = sorted(sources)[0]
                result[primary].add(rule)
                self.stats['duplicate'] += len(sources) - 1

        return result

    def extract_domains_from_rules(self, rules: List[str]) -> Set[str]:
        domains = set()
        for rule in rules:
            m = DOMAIN_RULE.match(rule)
            if m:
                domains.add(m.group(1))
                continue
            m = DOMAIN_SUFFIX_RULE.match(rule)
            if m:
                domains.add(m.group(1))
                continue
        return domains

    def filter_by_whitelist(self, rules: List[str], whitelist: Set[str]) -> List[str]:
        result = []
        for rule in rules:
            m = DOMAIN_RULE.match(rule) or DOMAIN_SUFFIX_RULE.match(rule)
            if m:
                domain = m.group(1).lower()
                if domain in whitelist:
                    continue
                if any(domain.endswith('.' + w) for w in whitelist):
                    continue
            result.append(rule)
        return result

    def split_by_type(self, rules: List[str]) -> Dict[str, List[str]]:
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
        return self.stats.copy()

    def reset_stats(self):
        self.stats = {
            'total': 0,
            'normalized': 0,
            'invalid': 0,
            'duplicate': 0,
        }


_default_processor = RuleProcessor()


def normalize_rule(line: str, source: str = "") -> Optional[str]:
    return _default_processor.normalize_rule(line, source)


def deduplicate_rules(rules: List[str]) -> List[str]:
    return _default_processor.deduplicate_rules(rules)
