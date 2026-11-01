/**
 * Sub-Store 精准聚合去广告脚本 (Loon / Egern)
 * 功能：支持合并 Loon 插件 (.plugin) 及 Surge 模块 (.sgmodule)
 * 转换 Surge URL Rewrite, Map Local 及 Script 格式至 Loon 兼容格式
 * 作者：Patatooo / ScriptHub-Automated
 */

function convertSurgeScriptToLoon(tag, line) {
    let params = {};
    let parts = line.split(',');
    for (let p of parts) {
        let idx = p.indexOf('=');
        if (idx !== -1) {
            let k = p.substring(0, idx).trim().toLowerCase();
            let v = p.substring(idx + 1).trim();
            params[k] = v;
        }
    }
    let type = params['type'] || 'http-response';
    let pattern = params['pattern'];
    let scriptPath = params['script-path'];
    if (!pattern || !scriptPath) return null;
    
    let loonParts = [];
    loonParts.push(`script-path=${scriptPath}`);
    loonParts.push(`tag=${tag}`);
    if (params['requires-body'] === '1' || params['requires-body'] === 'true') {
        loonParts.push('requires-body=true');
    }
    if (params['timeout']) {
        loonParts.push(`timeout=${params['timeout']}`);
    }
    if (params['argument']) {
        let argVal = params['argument'];
        // Remove quotes if any
        if (argVal.startsWith('"') && argVal.endsWith('"')) {
            argVal = argVal.slice(1, -1);
        }
        loonParts.push(`argument="${argVal}"`);
    }
    return `${type} ${pattern} ${loonParts.join(',')}`;
}

function mergeLoonPlugins(files) {
    const sections = {
        Rule: [],
        Rewrite: [],
        Script: [],
        MitM: new Set()
    };

    files.forEach(text => {
        if (typeof text !== 'string' || !text) return;

        let currentSection = null;
        let pluginName = "未知插件";
        
        const nameMatch = text.match(/^#!name=(.*)/m);
        if (nameMatch && nameMatch[1]) {
            pluginName = nameMatch[1].trim();
        }

        const lines = text.split(/\r?\n/);
        let hasAddedComment = { Rule: false, Rewrite: false, Script: false };

        for (let line of lines) {
            let trimmed = line.trim();
            if (!trimmed || trimmed.startsWith("#!")) continue;

            let sectionMatch = trimmed.match(/^\[(.*)\]/);
            if (sectionMatch) {
                let sec = sectionMatch[1].trim();
                let secLower = sec.toLowerCase();
                if (secLower === 'rule') {
                    currentSection = 'Rule';
                } else if (secLower === 'rewrite' || secLower === 'url rewrite' || secLower === 'map local' || secLower === 'body rewrite' || secLower === 'header rewrite') {
                    currentSection = 'Rewrite';
                } else if (secLower === 'script') {
                    currentSection = 'Script';
                } else if (secLower === 'mitm') {
                    currentSection = 'MitM';
                } else {
                    currentSection = null;
                }
                continue;
            }

            if (currentSection === "MitM") {
                if (/^hostname?\s*=/i.test(trimmed)) {
                    const rawHosts = trimmed.substring(trimmed.indexOf('=') + 1);
                    rawHosts.split(",").forEach(h => {
                        let hTrim = h.trim();
                        hTrim = hTrim.replace(/%APPEND%/g, "").replace(/%INSERT%/g, "").trim();
                        if (hTrim) sections.MitM.add(hTrim);
                    });
                }
            } else if (currentSection === "Rule") {
                if (!hasAddedComment.Rule) {
                    sections.Rule.push(`\n# > ${pluginName}`);
                    hasAddedComment.Rule = true;
                }
                sections.Rule.push(line);
            } else if (currentSection === "Rewrite") {
                if (!hasAddedComment.Rewrite) {
                    sections.Rewrite.push(`\n# > ${pluginName}`);
                    hasAddedComment.Rewrite = true;
                }
                let processedLine = line;
                if (/\s+-\s+reject/i.test(trimmed)) {
                    processedLine = line.replace(/\s+-\s+reject/i, ' reject');
                } else if (/\s+-\s+302\s+/i.test(trimmed)) {
                    processedLine = line.replace(/\s+-\s+302\s+/i, ' 302 ');
                } else if (/\s+-\s+307\s+/i.test(trimmed)) {
                    processedLine = line.replace(/\s+-\s+307\s+/i, ' 307 ');
                }
                sections.Rewrite.push(processedLine);
            } else if (currentSection === "Script") {
                if (!hasAddedComment.Script) {
                    sections.Script.push(`\n# > ${pluginName}`);
                    hasAddedComment.Script = true;
                }
                if (trimmed.includes('=') && !trimmed.startsWith('http-response') && !trimmed.startsWith('http-request') && !trimmed.startsWith('cron')) {
                    let idx = trimmed.indexOf('=');
                    let tag = trimmed.substring(0, idx).trim();
                    let content = trimmed.substring(idx + 1).trim();
                    let converted = convertSurgeScriptToLoon(tag, content);
                    if (converted) {
                        sections.Script.push(converted);
                    } else {
                        sections.Script.push(line);
                    }
                } else {
                    sections.Script.push(line);
                }
            }
        }
    });

    let output = [];
    output.push("#!name=聚合去广告大师 (Sub-Store 合并版)");
    output.push("#!desc=由 Sub-Store 自动提取合并，极大提升 Egern / Loon 加载速度");
    output.push("#!author=Patatooo | ScriptHub-Automated");
    output.push("#!icon=https://github.com/3183339668/Egern/raw/refs/heads/main/IMG_7064.jpeg");
    output.push("");

    if (sections.Rule.length > 0) {
        output.push("[Rule]");
        output.push(sections.Rule.join("\n").replace(/^\n/, ""));
        output.push("");
    }

    if (sections.Rewrite.length > 0) {
        output.push("[Rewrite]");
        output.push(sections.Rewrite.join("\n").replace(/^\n/, ""));
        output.push("");
    }

    if (sections.Script.length > 0) {
        output.push("[Script]");
        output.push(sections.Script.join("\n").replace(/^\n/, ""));
        output.push("");
    }

    if (sections.MitM.size > 0) {
        output.push("[MitM]");
        output.push(`hostname = ${Array.from(sections.MitM).join(", ")}`);
        output.push("");
    }

    return output.join("\n");
}

const contents = typeof $files !== 'undefined' ? $files : (typeof $content !== 'undefined' ? [$content] : []);
$content = mergeLoonPlugins(contents);
