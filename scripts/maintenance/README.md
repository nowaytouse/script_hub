# scripts/maintenance — 按需手动维护

不纳入 `main_update.py` 自动流水线；仅在需要时本地执行。

| 脚本 | 用途 |
|------|------|
| `merge_bilibili_bundle.py` | 从 BiliUniverse + Maasea 刷新 📺 BiliBili增强合集 |
| `merge_youtube_bundle.py` | 从 Maasea 刷新 📺 YouTube增强合集 |
| `merge_weibo_bundle.py` | 刷新 🐦 微博去广告合集 |
| `merge_apple_modules.py` | 合并 iRingo Maps/WeatherKit → Apple 服务增强合集 |
| `merge_dns_modules.py` | 合并 GetSomeFries DNS 上游到 DNS & Host Enhanced |
| `import_from_icloud_sr.py` | 从 iCloud 小火箭模块目录导入（需修改脚本内 `SR_DIR`） |
| `mitm_cleanup_github.py` | MITM hostname 加固（`main_update` 也会调用） |
| `localize_scripts.py` / `fix_module_categories.py` | 批量元数据修正 |

执行后请运行：

```bash
python3 scripts/consolidate_modules.py
python3 scripts/convert_surge_to_shadowrocket.py
```
