import re

filepath = "/Users/nyamiiko/Downloads/GitHub/script_hub/scripts/pipeline/adblock_manager.py"
with open(filepath, 'r') as f:
    content = f.read()

# 1. Modify active_rulesets for lite_only
content = content.replace(
    '        if lite_only:\n            active_rulesets = [p for p in generated_rulesets if self._category_from_filename(os.path.basename(p)) in LITE_CATEGORIES]',
    '        if lite_only:\n            # The user explicitly wants to KEEP privacy, threat intel, etc., in Lite.\n            active_rulesets = generated_rulesets'
)

# 2. Modify active_cats for lite_only
content = content.replace(
    '        active_cats = LITE_CATEGORIES if lite_only else set(self.category_names)',
    '        active_cats = set(self.category_names)'
)

# 3. Clear functional_sections for lite_only
# Find where all_sections is built
replace_block = """        all_sections = [("Rule", rule_lines)] + functional_sections
        all_sections = merge_mitm_hosts(all_sections)
        func_summary = ", ".join(func_counts) if func_counts else "无脚本层"

        if lite_only:
            name = f"📱 Universal Ad-Blocking Rules (PROMAX Lite) - [{current_date}]"
            desc = (
                f"手机轻量版({shard_count}片 REJECT 分片 + 应用内去广告脚本); "
                f"不含 ThreatIntel 重型规则; {func_summary}"
            )
            tag = "AdBlock, Lite, Mobile, HTTPDNS, Script"
        else:"""

new_block = """        if lite_only:
            all_sections = [("Rule", rule_lines)]
            func_summary = "无脚本/重写/MITM"
            name = f"📱 Universal Ad-Blocking Rules (PROMAX Lite) - [{current_date}]"
            desc = f"轻量基础版({shard_count}分片); 保留完整规则集与防火墙，纯净无脚本无MITM，性能拉满"
            tag = "AdBlock, Lite, Basic, Mobile"
        else:
            all_sections = [("Rule", rule_lines)] + functional_sections
            all_sections = merge_mitm_hosts(all_sections)
            func_summary = ", ".join(func_counts) if func_counts else "无脚本层"
            name = f"🚫 Universal Ad-Blocking Rules (PROMAX) - [{current_date}]"
            desc = (
                f"按用途分片({shard_count}片) + 应用内去广告({func_summary}); "
                f"索引 rulesets/AdBlock/catalog.json"
            )
            tag = "AdBlock, Dependency, HTTPDNS, Script"

        if not lite_only:
            pass # just a no-op so the else branch indentation from original code matches cleanly below. Actually wait..."""

content = re.sub(
    r'        all_sections = \[\("Rule", rule_lines\)\].*?else:',
    new_block,
    content,
    flags=re.DOTALL
)

with open(filepath, 'w') as f:
    f.write(content)
