import os
import re

files = [
    'rules_entrance.js', 'rules_landing.js', 'rules_relay.js',
    'singbox_entrance.js', 'singbox_landing.js', 'singbox_relay.js'
]

# Read the new mapping from singbox_entrance.js
with open('singbox_entrance.js', 'r', encoding='utf-8') as f:
    content = f.read()

mapping_match = re.search(r'(regionalCdnMapping:\s*\{.*?\n            \},)', content, re.DOTALL)
if not mapping_match:
    print("Could not find regionalCdnMapping in singbox_entrance.js")
    exit(1)
new_mapping = mapping_match.group(1)

for file in files:
    if not os.path.exists(file): continue
    print(f"Processing {file}...")
    with open(file, 'r', encoding='utf-8') as f:
        text = f.read()
    
    # 1. Update forceSniOverride
    text = re.sub(r'forceSniOverride:\s*false,', 'forceSniOverride: true,', text)
    
    # 2. Update regionalCdnMapping
    text = re.sub(r'(regionalCdnMapping:\s*\{.*?\n            \},)', new_mapping, text, flags=re.DOTALL)
    
    # 3. Update applyTlsConfig
    target1 = """                if (cfg.forceSniOverride || !proxy.sni) {
                    const connectionSafeSni = getConnectionSafeSni(proxy, regionName);
                    if (connectionSafeSni) {
                        applySecureSniToProxy(proxy, connectionSafeSni);
                    }
                }"""
    replace1 = """                if (cfg.forceSniOverride || !proxy.sni) {
                    const connectionSafeSni = getConnectionSafeSni(proxy, regionName);
                    if (connectionSafeSni) {
                        applySecureSniToProxy(proxy, connectionSafeSni);
                        // ⚠️ 强制替换 SNI 后，必须关闭证书验证，否则会发生证书不匹配错误
                        proxy['skip-cert-verify'] = true;
                        proxy['_force_strict_tls_verify'] = false;
                    }
                }"""
    if "proxy['_force_strict_tls_verify'] = false;" not in text:
        text = text.replace(target1, replace1)
    
    # 4. Update applySmartTlsEnhancement
    target2 = """                // SNI：仅在未设置时添加
                if (!proxy.sni && !proxy.flow) {
                    const connectionSafeSni = getConnectionSafeSni(proxy, regionName);
                    if (connectionSafeSni) {
                        applySecureSniToProxy(proxy, connectionSafeSni);
                    }
                }"""
    replace2 = """                // SNI：仅在未设置时添加，或开启强制覆盖时
                if (cfg.forceSniOverride || (!proxy.sni && !proxy.flow)) {
                    const connectionSafeSni = getConnectionSafeSni(proxy, regionName);
                    if (connectionSafeSni) {
                        applySecureSniToProxy(proxy, connectionSafeSni);
                        // ⚠️ 强制替换 SNI 后，必须关闭证书验证，否则会发生证书不匹配错误
                        proxy['skip-cert-verify'] = true;
                        proxy['_force_strict_tls_verify'] = false;
                    }
                }"""
    text = text.replace(target2, replace2)
    
    # 5. Update https case
    target3 = """                    // 智能 SNI 配置
                    if (!modifiedProxy.sni || cfg.forceSniOverride) {
                        const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '', modifiedProxy.server);
                        const connectionSafeSni = getConnectionSafeSni(modifiedProxy, regionInfo.r);
                        if (connectionSafeSni) {
                            applySecureSniToProxy(modifiedProxy, connectionSafeSni);
                        }
                    }"""
    replace3 = """                    // 智能 SNI 配置
                    if (!modifiedProxy.sni || cfg.forceSniOverride) {
                        const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '', modifiedProxy.server);
                        const connectionSafeSni = getConnectionSafeSni(modifiedProxy, regionInfo.r);
                        if (connectionSafeSni) {
                            applySecureSniToProxy(modifiedProxy, connectionSafeSni);
                            // ⚠️ 强制替换 SNI 后，必须关闭证书验证，否则会发生证书不匹配错误
                            modifiedProxy['skip-cert-verify'] = true;
                            modifiedProxy['_force_strict_tls_verify'] = false;
                        }
                    }"""
    text = text.replace(target3, replace3)
    
    with open(file, 'w', encoding='utf-8') as f:
        f.write(text)

print("Done!")
