log(`🚀 开始三跳链式代理节点插入脚本处理`);

// ==================== 参数解析 ====================
// 第一跳参数（节点入口）
let { 
    name1, outbound1, type1, includeUnsupportedProxy1, url1,
    name2, outbound2, type2, includeUnsupportedProxy2, url2,
    name3, outbound3, type3, includeUnsupportedProxy3, url3
} = $arguments;

log(`\n📋 三跳配置参数:`);
log(`  第一跳(入口): name=${name1}, type=${type1}`);
log(`  第二跳(中续): name=${name2}, type=${type2}`);
log(`  第三跳(落地): name=${name3}, type=${type3}`);

const parser = ProxyUtils.JSON5 || JSON;
log(`\n使用 ${ProxyUtils.JSON5 ? 'JSON5' : 'JSON'} 解析配置文件`);

let config;
try {
    config = parser.parse($content ?? $files[0]);
} catch (e) {
    log(`❌ 解析失败: ${e.message ?? e}`);
    throw new Error(`配置文件不是合法的 ${ProxyUtils.JSON5 ? 'JSON5' : 'JSON'} 格式`);
}

// ==================== 性能优化：预编译正则表达式 ====================
const SANITIZE_REGEX = /[\[\]【】"']+/g;
const WHITESPACE_REGEX = /\s+/g;
const SANITIZE_CACHE = new Map();

// ==================== 节点名称清理函数 ====================
function sanitizeNodeTag(tag) {
    if (!tag) return tag;
    if (SANITIZE_CACHE.has(tag)) return SANITIZE_CACHE.get(tag);
    
    const cleaned = tag.replace(SANITIZE_REGEX, '').replace(/[\t\n\r]/g, ' ').replace(/ {3,}/g, ' ').trimEnd();
    SANITIZE_CACHE.set(tag, cleaned);
    return cleaned;
}

// ==================== 获取订阅节点函数 ====================
async function fetchProxies(name, type, url, includeUnsupportedProxy, hopLabel) {
    log(`\n📥 ${hopLabel}: 获取订阅节点...`);
    
    const typeValue = /^1$|col|组合/i.test(type) ? 'collection' : 'subscription';
    
    let proxies;
    if (url) {
        log(`  从 URL 读取订阅: ${url}`);
        proxies = await produceArtifact({
            name,
            type: typeValue,
            platform: 'sing-box',
            produceType: 'internal',
            produceOpts: {
                'include-unsupported-proxy': includeUnsupportedProxy,
            },
            subscription: {
                name,
                url,
                source: 'remote',
            },
        });
    } else {
        log(`  读取订阅: ${name} (${typeValue === 'collection' ? '组合' : '单个'})`);
        proxies = await produceArtifact({
            name,
            type: typeValue,
            platform: 'sing-box',
            produceType: 'internal',
            produceOpts: {
                'include-unsupported-proxy': includeUnsupportedProxy,
            },
        });
    }
    
    // 清理所有代理节点名称（批量处理）
    for (let i = 0; i < proxies.length; i++) {
        proxies[i].tag = sanitizeNodeTag(proxies[i].tag);
    }
    
    log(`  ✅ 获取到 ${proxies.length} 个节点`);
    
    // 显示节点示例
    if (proxies.length > 0) {
        log(`  📋 节点示例（前5个）:`);
        proxies.slice(0, 5).forEach((proxy, idx) => {
            log(`    ${idx + 1}. ${proxy.tag} (${proxy.type})`);
        });
        if (proxies.length > 5) {
            log(`    ... 还有 ${proxies.length - 5} 个节点`);
        }
    }
    
    return proxies;
}

// ==================== 解析 outbound 规则函数 ====================
function parseOutboundRules(outbound, hopLabel) {
    log(`\n🔍 ${hopLabel}: 解析插入规则...`);
    
    if (!outbound) {
        log(`  ⚠️ 未配置 outbound 参数，跳过`);
        return [];
    }
    
    const outbounds = outbound
        .split('🕳')
        .filter(i => i)
        .map(i => {
            let [outboundPattern, tagPattern = '.*'] = i.split('🏷');
            const tagRegex = createTagRegExp(tagPattern);
            log(`  规则: 节点匹配 [${tagPattern}] ➜ 插入到 [${outboundPattern}]`);
            return [outboundPattern, tagRegex];
        });
    
    log(`  ✅ 共 ${outbounds.length} 条插入规则`);
    return outbounds;
}

// ==================== 插入节点到策略组函数（优化版本） ====================
function insertProxiesToGroups(proxies, outbounds, hopLabel, stats) {
    log(`\n📝 ${hopLabel}: 插入节点到策略组...`);
    let insertedCount = 0;
    const VALID_TYPES = new Set(['selector', 'urltest']);
    
    // 预编译所有正则表达式
    const compiledRules = outbounds.map(([pattern, tagRegex]) => ({
        outboundRegex: createOutboundRegExp(pattern),
        tagRegex
    }));
    
    for (let i = 0; i < config.outbounds.length; i++) {
        const outbound = config.outbounds[i];
        
        for (let j = 0; j < compiledRules.length; j++) {
            const { outboundRegex, tagRegex } = compiledRules[j];
            
            if (!outboundRegex.test(outbound.tag)) continue;
            if (!VALID_TYPES.has(outbound.type)) continue;
            
            if (!Array.isArray(outbound.outbounds)) {
                outbound.outbounds = [];
            }
            
            const matchedTags = getTags(proxies, tagRegex);
            
            if (!stats[outbound.tag]) {
                stats[outbound.tag] = {
                    before: outbound.outbounds.length,
                    inserted: 0,
                    nodes: [],
                    hop: hopLabel
                };
            }
            
            if (matchedTags.length > 0) {
                stats[outbound.tag].inserted += matchedTags.length;
                stats[outbound.tag].nodes.push(...matchedTags);
                insertedCount += matchedTags.length;
                outbound.outbounds.push(...matchedTags);
            }
        }
    }
    
    log(`  ✅ 本跳共插入 ${insertedCount} 个节点`);
    return insertedCount;
}

// ==================== 主流程 ====================
const allProxies = [];
const insertionStats = {};
let totalInserted = 0;

// 第一跳：节点入口
if (name1 && outbound1) {
    const proxies1 = await fetchProxies(name1, type1, url1, includeUnsupportedProxy1, '第一跳(入口)');
    const outbounds1 = parseOutboundRules(outbound1, '第一跳(入口)');
    const inserted1 = insertProxiesToGroups(proxies1, outbounds1, '第一跳(入口)', insertionStats);
    totalInserted += inserted1;
    allProxies.push(...proxies1);
}

// 第二跳：中续路径
if (name2 && outbound2) {
    const proxies2 = await fetchProxies(name2, type2, url2, includeUnsupportedProxy2, '第二跳(中续)');
    const outbounds2 = parseOutboundRules(outbound2, '第二跳(中续)');
    const inserted2 = insertProxiesToGroups(proxies2, outbounds2, '第二跳(中续)', insertionStats);
    totalInserted += inserted2;
    allProxies.push(...proxies2);
}

// 第三跳：落地节点
if (name3 && outbound3) {
    const proxies3 = await fetchProxies(name3, type3, url3, includeUnsupportedProxy3, '第三跳(落地)');
    const outbounds3 = parseOutboundRules(outbound3, '第三跳(落地)');
    const inserted3 = insertProxiesToGroups(proxies3, outbounds3, '第三跳(落地)', insertionStats);
    totalInserted += inserted3;
    allProxies.push(...proxies3);
}

log(`\n✅ 三跳总共插入 ${totalInserted} 个节点`);

// ==================== 空策略组检查 ====================
log(`\n🔍 检查空策略组...`);

const compatible_outbound = {
    tag: 'COMPATIBLE',
    type: 'direct',
};

let compatibleAdded = false;

config.outbounds.forEach(outbound => {
    if ((outbound.type === 'selector' || outbound.type === 'urltest') && 
        Array.isArray(outbound.outbounds) && 
        outbound.outbounds.length === 0) {
        
        if (!compatibleAdded) {
            config.outbounds.push(compatible_outbound);
            compatibleAdded = true;
            log(`  ➕ 添加兜底节点: COMPATIBLE (direct)`);
        }
        log(`  ⚠️ [${outbound.tag}] 为空，插入 COMPATIBLE`);
        outbound.outbounds.push(compatible_outbound.tag);
    }
});

// ==================== 验证节点唯一性（优化版本） ====================
log(`\n🔍 验证节点唯一性...`);
const tagCount = new Map();
const outboundsLen = config.outbounds.length;
const allProxiesLen = allProxies.length;

// 一次遍历统计所有标签
for (let i = 0; i < outboundsLen; i++) {
    const tag = config.outbounds[i].tag;
    tagCount.set(tag, (tagCount.get(tag) || 0) + 1);
}
for (let i = 0; i < allProxiesLen; i++) {
    const tag = allProxies[i].tag;
    tagCount.set(tag, (tagCount.get(tag) || 0) + 1);
}

const duplicates = [];
for (const [tag, count] of tagCount) {
    if (count > 1) {
        duplicates.push({ tag, count });
    }
}

if (duplicates.length > 0) {
    log(`  ⚠️ 发现 ${duplicates.length} 个重复节点名称:`);
    const showCount = Math.min(5, duplicates.length);
    for (let i = 0; i < showCount; i++) {
        log(`     • ${duplicates[i].tag} (${duplicates[i].count}次)`);
    }
    if (duplicates.length > 5) {
        log(`     ... 还有 ${duplicates.length - 5} 个重复`);
    }
    log(`  这些重复将在合成脚本中处理`);
} else {
    log(`  ✅ 所有节点名称唯一`);
}

// ==================== 添加代理节点到配置 ====================
log(`\n📥 添加代理节点到配置...`);
config.outbounds.push(...allProxies);
log(`✅ 已添加 ${allProxies.length} 个代理节点`);

// ==================== 最终统计 ====================
log(`\n📊 最终统计:`);
log(`  ┌─ 原有 outbound: ${config.outbounds.length - allProxies.length - (compatibleAdded ? 1 : 0)}`);
log(`  ├─ 新增代理节点: ${allProxies.length}`);
log(`  │  ├─ 第一跳(入口): ${name1 ? '已配置' : '未配置'}`);
log(`  │  ├─ 第二跳(中续): ${name2 ? '已配置' : '未配置'}`);
log(`  │  └─ 第三跳(落地): ${name3 ? '已配置' : '未配置'}`);
if (compatibleAdded) {
    log(`  ├─ 兜底节点: 1 (COMPATIBLE)`);
}
log(`  └─ 总计 outbound: ${config.outbounds.length}`);

log(`\n📋 策略组插入详情:`);
Object.entries(insertionStats).forEach(([tag, stats]) => {
    if (stats.inserted > 0) {
        log(`  ${tag} [${stats.hop}]:`);
        log(`    ├─ 原有: ${stats.before} 个`);
        log(`    ├─ 新增: ${stats.inserted} 个`);
        log(`    └─ 现有: ${stats.before + stats.inserted} 个`);
    }
});

log(`\n✅ 三跳节点插入脚本处理完成`);

// ==================== 第二阶段：去重和链式代理 ====================
log(`\n\n${'='.repeat(60)}`);
log(`🔄 开始第二阶段：去重和链式代理处理`);
log(`${'='.repeat(60)}\n`);

// ==================== 链式代理配置 ====================
// 三跳链路: 入口 → 中续 → 落地
// 注意: 名称必须与配置文件中的outbound tag完全匹配
const relay = {
    '♻️ 自动入口 🧠': '🚶 中续路径 🔐',      // 入口节点 → 中续节点
    '🚶 中续路径 🔐': '🕳️ 落地节点 🔐 +',   // 中续节点 → 落地节点
};

log(`📋 链式代理配置:`);
Object.entries(relay).forEach(([from, to]) => {
    log(`   ${from} ➜ ${to}`);
});
log('');

// ==================== 去重核心函数 ====================
function sanitizeTag(tag) {
    if (!tag) return tag;
    return tag.replace(/[\[\]【】"']/g, '').replace(/[\t\n\r]/g, ' ').replace(/ {3,}/g, ' ').trimEnd();
}

function robustDeduplicateOutbounds(outbounds) {
    log(`🔍 步骤1: 去重和清理节点标签（防碰撞模式）...`);
    
    const finalTags = new Set();
    const tagCounters = new Map();
    const sanitizedToFinalsMap = new Map();
    const len = outbounds.length;

    // 单次遍历完成清理、去重和映射
    for (let i = 0; i < len; i++) {
        const outbound = outbounds[i];
        const original = outbound.tag;
        const sanitized = sanitizeTag(original);
        
        let finalTag = sanitized;
        let counter = tagCounters.get(sanitized) || 1;

        while (finalTags.has(finalTag)) {
            finalTag = `${sanitized} #${counter}`;
            counter++;
        }
        
        tagCounters.set(sanitized, counter);
        finalTags.add(finalTag);
        outbound.tag = finalTag;
        
        // 构建映射
        if (!sanitizedToFinalsMap.has(sanitized)) {
            sanitizedToFinalsMap.set(sanitized, []);
        }
        sanitizedToFinalsMap.get(sanitized).push(finalTag);
    }

    log(`✅ 节点去重完成`);
    return { sanitizedToFinalsMap };
}

function updateReferences(config, sanitizedToFinalsMap) {
    log(`🔍 步骤2: 更新策略组中的节点引用...`);
    const allFinalTags = new Set();
    const len = config.outbounds.length;
    
    // 预构建标签集合
    for (let i = 0; i < len; i++) {
        allFinalTags.add(config.outbounds[i].tag);
    }

    for (let i = 0; i < len; i++) {
        const outbound = config.outbounds[i];
        
        if (Array.isArray(outbound.outbounds)) {
            const newOutbounds = [];
            const seenTags = new Set();
            const memberLen = outbound.outbounds.length;
            
            for (let j = 0; j < memberLen; j++) {
                const oldTag = outbound.outbounds[j];
                const sanitizedOldTag = sanitizeTag(oldTag);
                const resolvedTags = sanitizedToFinalsMap.get(sanitizedOldTag);
                
                if (resolvedTags) {
                    for (const tag of resolvedTags) {
                        if (allFinalTags.has(tag) && !seenTags.has(tag)) {
                            newOutbounds.push(tag);
                            seenTags.add(tag);
                        }
                    }
                } else if (allFinalTags.has(sanitizedOldTag) && !seenTags.has(sanitizedOldTag)) {
                    newOutbounds.push(sanitizedOldTag);
                    seenTags.add(sanitizedOldTag);
                } else if (allFinalTags.has(oldTag) && !seenTags.has(oldTag)) {
                    newOutbounds.push(oldTag);
                    seenTags.add(oldTag);
                }
            }
            outbound.outbounds = newOutbounds;
        }
        
        if (outbound.default) {
            const sanitizedDefault = sanitizeTag(outbound.default);
            const resolvedDefaults = sanitizedToFinalsMap.get(sanitizedDefault);
            if (resolvedDefaults && resolvedDefaults.length > 0) {
                outbound.default = resolvedDefaults[0];
            } else if (allFinalTags.has(sanitizedDefault)) {
                outbound.default = sanitizedDefault;
            }
        }
    }
    log(`✅ 引用更新完成`);
}

// ==================== 执行去重和链式代理 ====================
const { sanitizedToFinalsMap } = robustDeduplicateOutbounds(config.outbounds);
updateReferences(config, sanitizedToFinalsMap);

log(`🔍 步骤3-5: 清理字段、识别策略组...`);
const groupTypes = new Set(['urltest', 'selector', 'load-balance']);
const noDetourTypes = new Set(['direct', 'block', 'dns']);
const strategyGroups = new Set();
const len = config.outbounds.length;

// 单次遍历完成多个操作
for (let i = 0; i < len; i++) {
    const outbound = config.outbounds[i];
    
    if (!groupTypes.has(outbound.type)) {
        if (outbound.outbounds) delete outbound.outbounds;
    } else {
        strategyGroups.add(outbound.tag);
    }
    
    if (outbound.detour) delete outbound.detour;
}

log(`📊 识别到 ${strategyGroups.size} 个策略组`);

log(`🔍 步骤6: 宏观级别循环检测...`);
const initialChains = new Map();
for (const [source, target] of Object.entries(relay)) {
    const sourceTag = sanitizeTag(source);
    const targetTag = sanitizeTag(target);

    if (strategyGroups.has(sourceTag) && strategyGroups.has(targetTag)) {
        initialChains.set(sourceTag, targetTag);
    }
}

const visiting = new Set();
const visited = new Set();
const safeChains = new Map(initialChains);
let cycleFoundInRelay = false;

function detectRelayCycle(group, path = []) {
    visiting.add(group);
    const target = initialChains.get(group);
    if (target) {
        const newPath = [...path, group];
        if (visiting.has(target)) {
            const cyclePath = [...newPath, target].join(' ➜ ');
            log(`   ❌ 检测到宏观循环: \${cyclePath}`);
            log(`   🛡️ 为防止错误，此链接将被断开: \${group} -> \${target}`);
            safeChains.delete(group);
            cycleFoundInRelay = true;
        } else if (!visited.has(target)) {
            detectRelayCycle(target, newPath);
        }
    }
    visiting.delete(group);
    visited.add(group);
}

initialChains.forEach((_, source) => {
    if (!visited.has(source)) detectRelayCycle(source);
});

if (!cycleFoundInRelay) {
    log(`✅ 未检测到宏观循环`);
} else {
    log(`⚠️ 检测到宏观循环并已断开`);
}
const validChains = safeChains;

log(`🔍 步骤7: 微观级别循环检测...`);
const groupToRealNodesMap = new Map();

// 预构建outbound索引
const outboundIndex = new Map();
for (let i = 0; i < config.outbounds.length; i++) {
    outboundIndex.set(config.outbounds[i].tag, config.outbounds[i]);
}

function resolveGroupNodes(groupTag, path = new Set()) {
    if (groupToRealNodesMap.has(groupTag)) return groupToRealNodesMap.get(groupTag);
    if (path.has(groupTag)) return new Set();
    path.add(groupTag);
    
    const group = outboundIndex.get(groupTag);
    const realNodes = new Set();
    if (group && Array.isArray(group.outbounds)) {
        const memberLen = group.outbounds.length;
        for (let i = 0; i < memberLen; i++) {
            const childTag = group.outbounds[i];
            if (strategyGroups.has(childTag)) {
                const childNodes = resolveGroupNodes(childTag, new Set(path));
                for (const node of childNodes) {
                    realNodes.add(node);
                }
            } else {
                realNodes.add(childTag);
            }
        }
    }
    groupToRealNodesMap.set(groupTag, realNodes);
    return realNodes;
}

for (const groupTag of strategyGroups) {
    resolveGroupNodes(groupTag);
}
log(`✅ 节点所有权预计算完成`);

log(`🔍 步骤8: 设置链式代理（detours）...`);
let chainedCount = 0;
const chainDetails = {};

for (const [sourceGroup, targetGroup] of validChains) {
    const sourceRealNodes = groupToRealNodesMap.get(sourceGroup) || new Set();
    const targetRealNodes = groupToRealNodesMap.get(targetGroup) || new Set();
    
    if (sourceRealNodes.size === 0) {
        continue;
    }

    let groupChainedCount = 0;
    for (const nodeTag of sourceRealNodes) {
        const node = outboundIndex.get(nodeTag);
        if (!node || noDetourTypes.has(node.type)) continue;
        
        if (targetRealNodes.has(nodeTag)) {
            continue;
        }
        
        node.detour = targetGroup;
        groupChainedCount++;
        if (!chainDetails[sourceGroup]) chainDetails[sourceGroup] = [];
        chainDetails[sourceGroup].push({ node: nodeTag, via: targetGroup });
    }
    
    chainedCount += groupChainedCount;
}

// ==================== 最终报告 ====================
log(`${'='.repeat(60)}`);
log(`📊 最终处理报告`);
log(`${'='.repeat(60)}`);
if (chainedCount > 0) {
    log(`✅ 成功为 ${chainedCount} 个节点设置链式代理`);
    log(`🔗 链式详情:`);
    for (const [group, nodes] of Object.entries(chainDetails)) {
        log(`   ${group} (${nodes.length} 个节点):`);
        const showCount = Math.min(5, nodes.length);
        for (let i = 0; i < showCount; i++) {
            log(`     ├─ ${nodes[i].node} ➜ ${nodes[i].via}`);
        }
        if (nodes.length > 5) log(`     └─ ... 还有 ${nodes.length - 5} 个`);
    }
} else {
    log(`⚠️ 本次未设置任何链式代理`);
    log(`   请检查您的中继配置`);
}
log(`${'='.repeat(60)}`);
log(`✅ 处理完成`);

$content = JSON.stringify(config, null, 2);

// ==================== 辅助函数 ====================
function getTags(proxies, regex) {
    return (regex ? proxies.filter(p => regex.test(p.tag)) : proxies).map(p => p.tag);
}

function log(v) {
    console.log(`[📦 三跳填充] ${v}`);
}

function createTagRegExp(tagPattern) {
    return new RegExp(tagPattern.replace(/ℹ️/g, '').trim(), tagPattern.includes('ℹ️') ? 'i' : undefined);
}

function createOutboundRegExp(outboundPattern) {
    return new RegExp(outboundPattern.replace(/ℹ️/g, '').trim(), outboundPattern.includes('ℹ️') ? 'i' : undefined);
}
