# 归档脚本说明

## 目录结构

### `/verification/` - 验证类脚本
用于检查配置、模块、规则集的正确性，手动运行。

- `test_singbox_startup.sh` - 测试 SingBox 启动
- `validate_singbox_config.sh` - 验证 SingBox 配置
- `verify_all_configs_public.sh` - 验证所有公开配置
- `verify_modules.sh` - 验证模块
- `verify_modules_simple.sh` - 简单模块验证
- `verify_region_rules.sh` - 验证地区规则
- `check_duplicates.py` - 检查重复项
- `verify_categories.py` - 验证分类

### `/legacy/` - 旧版脚本
已被新脚本替代或不再使用的脚本。

- `merge_all_rulesets.sh` - 被 `incremental_merge_all.sh` 替代
- `sync_all_rules.sh` - 旧版同步脚本
- `ruleset_merger.sh` - 旧版规则集合并器
- `sync_modules_to_shadowrocket.sh` - 旧版模块同步
- `check_empty_rulesets.sh` - 被 `ruleset_cleaner.sh` 替代

### `/tools/` - 工具类脚本
手动使用的工具脚本，不在自动更新流程中。

- `PURGE_PRIVATE_HISTORY.sh` - 清理私有历史
- `clean_git_history.sh` - 清理 Git 历史
- `convert_to_module.sh` - 转换为模块
- `organize_modules_by_group.sh` - 按组织模块
- `fix_module_categories.sh` - 修复模块分类
- `check_config_sync.sh` - 检查配置同步

## 使用说明

- **验证脚本**: 在修改配置后手动运行检查
- **旧版脚本**: 保留作为参考，不建议使用
- **工具脚本**: 根据需要手动运行
