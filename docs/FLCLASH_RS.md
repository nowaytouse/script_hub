# FlClash RS and Surge modules

[English](FLCLASH_RS.md) · [简体中文](FLCLASH_RS.zh-CN.md)

In Script Hub, open **Modules → Surge / FlClash** and copy a Surge
`.sgmodule` link. In FlClash RS, import it under **MITM → Modules → Add**.
Review the effective and ignored entries and confirm that the rule sets
downloaded successfully. For HTTPS rewrites, follow the client's certificate
trust and script authorization flows.

The shared PROMAX module is under 1 MB and loads its AdBlock rule sets
separately. Replace any previous PROMAX entry with the shared link and keep
one active PROMAX module. Use **Update** in FlClash RS to refresh it.
[Shared Surge PROMAX](https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/surge/head_expanse/github/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule)

Use the [FlClash YAML template](TEMPLATES.md) for profile routing. To add the
complete 217heidai source to profile routing, load the Mihomo provider
from the [multi-client bundle](COMPATIBILITY.md#complete-217heidai-input).

[FlClash RS build](https://github.com/chen08209/FlClash/actions/runs/44287831009) · [Rule-set importer tests](https://github.com/chen08209/FlClash/blob/e06994ed6e15c6e80f8691cc4f933d34d8d60819/test/common/mitm_module_test.dart#L466-L525)
