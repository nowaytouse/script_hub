/**
 * ============================================================
 * Sub-Store Entrance Refactor
 * Version: v4.0.0
 * Updated: 2026-04-27
 * Scope: VLESS / Hysteria2 only
 * ============================================================
 *
 * Design Goals
 * 1. Keep node availability first.
 * 2. Prefer IPv6 and modern TLS defaults.
 * 3. Force SNI/Host unification, but only within handshake-safe bounds.
 * 4. Remove script-side overlap with Sub-Store built-in switches:
 *    UDP relay / TCP Fast Open / VMess AEAD.
 *    skip-cert-verify is emitted only for forced SNI rewrites that
 *    would otherwise reduce availability.
 * 5. Stealth preference:
 *    - prefer carrier / game / telemetry style domains
 *    - avoid obvious foreign CDN / cloud root domains when a better
 *      existing candidate already exists inside the same node
 */

const PROTOCOL_WHITELIST = Object.freeze(new Set(['vless', 'hysteria2']));

const MANAGED_SWITCH_KEYS = Object.freeze([
    'udp', 'udp-relay', 'udp-over-tcp', 'uot', 'xudp', 'packet-encoding',
    'tfo', 'fast-open', 'skip-cert-verify', 'vmess-aead', 'aead'
]);

const TEMP_KEYS = Object.freeze([
    '_sourceIndex', '_regionPriority', '_regionName', '_regionFlag', '_originalName',
    '_forceSkipCertVerify'
]);

const TRACKING_FIELDS = Object.freeze([
    '_subscription_url', '_sub_url', '_provider_url', '_source_url',
    '_panel_url', '_api_url', '_update_url', '_download_url',
    '_traffic_info', '_expire_info', '_user_info', '_account_info'
]);

const PROVIDER_TRACKING_HEADERS = Object.freeze(new Set([
    'authorization', 'x-api-user', 'x-user-id', 'x-subscription-id',
    'x-sub-id', 'x-provider', 'x-provider-info', 'x-source',
    'x-account', 'x-panel-id', 'x-client-id', 'x-auth-token',
    'x-access-token', 'x-membership', 'x-plan', 'x-tier',
    'x-subscription-info', 'x-traffic-used', 'x-traffic-total',
    'x-expire-date', 'x-renewal-date', 'x-affiliate', 'x-referral',
    'x-invite-code', 'x-custom-header', 'x-tracking-id'
]));

const BLOCK_TERMS = Object.freeze([
    '剩余', '到期', '流量', '官网', '客服', '购买', '续费', '订阅', '重置', '充值',
    '防失联', '电报', 'telegram', '广告', '推广', '宣传', 'aff', 'invite', 'promo',
    '试用', '体验', '失效', '过期', '停用', '不可用', '超时', '失败', '错误', '被拒',
    '测试', '测速', '故障', '维护', '升级', '异常', 'backup', 'trial', 'demo',
    'invalid', 'offline', 'maintenance', 'expired', 'disabled', 'banned'
]);

const REGION_PATTERNS = Object.freeze([
    { flag: '🇭🇰', region: '香港', short: 'HK', priority: 10, pattern: /香港|hong\s*kong|(?:^|[\s\-·])hk(?:[\s\-·]|$)|hkt|hgc|pccw|wtt|csl|hkbn/i },
    { flag: '🇹🇼', region: '台湾', short: 'TW', priority: 11, pattern: /台湾|台灣|taiwan|(?:^|[\s\-·])tw(?:[\s\-·]|$)|taipei|hinet|cht/i },
    { flag: '🇯🇵', region: '日本', short: 'JP', priority: 12, pattern: /日本|japan|(?:^|[\s\-·])jp(?:[\s\-·]|$)|tokyo|osaka|ntt|iij|kddi|softbank/i },
    { flag: '🇰🇷', region: '韩国', short: 'KR', priority: 13, pattern: /韩国|韓國|korea|(?:^|[\s\-·])kr(?:[\s\-·]|$)|seoul|sk(?:[\s\-·]|$)|kt(?:[\s\-·]|$)|lg\s*u/i },
    { flag: '🇸🇬', region: '新加坡', short: 'SG', priority: 14, pattern: /新加坡|singapore|(?:^|[\s\-·])sg(?:[\s\-·]|$)|singtel|starhub|m1/i },
    { flag: '🇲🇴', region: '澳门', short: 'MO', priority: 15, pattern: /澳门|澳門|macau|macao|(?:^|[\s\-·])mo(?:[\s\-·]|$)|ctm/i },
    { flag: '🇨🇳', region: '中国', short: 'CN', priority: 20, pattern: /中国|mainland|(?:^|[\s\-_])cn(?:[\s\-_]|$)|北京|上海|广州|深圳|杭州|南京|成都|武汉|移动|联通|电信|cmcc|cucc|ctcc/i },
    { flag: '🇺🇸', region: '美国', short: 'US', priority: 30, pattern: /美国|美國|united\s*states|(?:^|[\s\-·])us(?:[\s\-·]|$)|usa|los\s*angeles|san\s*jose|new\s*york|ashburn|seattle|dallas/i },
    { flag: '🇨🇦', region: '加拿大', short: 'CA', priority: 31, pattern: /加拿大|canada|(?:^|[\s\-·])ca(?:[\s\-·]|$)|toronto|vancouver|montreal/i },
    { flag: '🇬🇧', region: '英国', short: 'UK', priority: 40, pattern: /英国|英國|united\s*kingdom|(?:^|[\s\-·])uk(?:[\s\-·]|$)|london|manchester/i },
    { flag: '🇩🇪', region: '德国', short: 'DE', priority: 41, pattern: /德国|德國|germany|(?:^|[\s\-·])de(?:[\s\-·]|$)|frankfurt|berlin|munich/i },
    { flag: '🇫🇷', region: '法国', short: 'FR', priority: 42, pattern: /法国|法國|france|(?:^|[\s\-·])fr(?:[\s\-·]|$)|paris|marseille/i },
    { flag: '🇳🇱', region: '荷兰', short: 'NL', priority: 43, pattern: /荷兰|荷蘭|netherlands|(?:^|[\s\-·])nl(?:[\s\-·]|$)|amsterdam|rotterdam/i },
    { flag: '🇦🇺', region: '澳洲', short: 'AU', priority: 44, pattern: /澳洲|澳大利亚|australia|(?:^|[\s\-·])au(?:[\s\-·]|$)|sydney|melbourne|brisbane/i },
    { flag: '🇲🇾', region: '马来西亚', short: 'MY', priority: 45, pattern: /马来西亚|馬來西亞|malaysia|(?:^|[\s\-·])my(?:[\s\-·]|$)|kuala/i },
    { flag: '🇹🇭', region: '泰国', short: 'TH', priority: 46, pattern: /泰国|泰國|thailand|(?:^|[\s\-·])th(?:[\s\-·]|$)|bangkok/i },
    { flag: '🇻🇳', region: '越南', short: 'VN', priority: 47, pattern: /越南|vietnam|(?:^|[\s\-·])vn(?:[\s\-·]|$)|hanoi|ho\s*chi/i },
    { flag: '🇵🇭', region: '菲律宾', short: 'PH', priority: 48, pattern: /菲律宾|philippines|(?:^|[\s\-·])ph(?:[\s\-·]|$)|manila/i },
    { flag: '🇮🇩', region: '印尼', short: 'ID', priority: 49, pattern: /印尼|印度尼西亚|indonesia|(?:^|[\s\-·])id(?:[\s\-·]|$)|jakarta/i }
]);

const FINGERPRINT_POOLS = Object.freeze({
    default: ['chrome', 'safari', 'edge', 'firefox'],
    '中国': ['chrome', 'safari', 'edge'],
    '香港': ['chrome', 'safari', 'edge', 'firefox'],
    '台湾': ['chrome', 'safari', 'edge', 'firefox'],
    '日本': ['chrome', 'safari', 'edge', 'firefox'],
    '韩国': ['chrome', 'safari', 'edge', 'firefox'],
    '新加坡': ['chrome', 'edge', 'safari', 'firefox']
});

const COVER_POOLS = Object.freeze({
    default: [
        'game.gtimg.cn',
        'imgcache.qq.com',
        'dlied1.qq.com',
        'wegame.gtimg.com',
        'pgdt.gtimg.cn',
        'res.wx.qq.com',
        'adash.m.taobao.com',
        'log.mmstat.com',
        'gw.alicdn.com',
        'sdkconfig.ad.xiaomi.com',
        'tracking.miui.com',
        'img.client.10010.com',
        'm.client.10010.com',
        'app.10086.cn',
        'open.e.189.cn',
        'cloud.189.cn',
        'res.vivo.com.cn',
        'res.oppomobile.com',
        'api.m.mi.com',
        'webstatic.mihoyo.com',
        'log-upload-os.hoyoverse.com',
        'cdn.4399.com'
    ],
    '香港': ['game.gtimg.cn', 'wegame.gtimg.com', 'imgcache.qq.com', 'gw.alicdn.com', 'img.client.10010.com', 'app.10086.cn'],
    '台湾': ['game.gtimg.cn', 'wegame.gtimg.com', 'imgcache.qq.com', 'gw.alicdn.com', 'res.vivo.com.cn', 'api.m.mi.com'],
    '日本': ['game.gtimg.cn', 'wegame.gtimg.com', 'dlied1.qq.com', 'imgcache.qq.com', 'webstatic.mihoyo.com', 'cdn.4399.com'],
    '韩国': ['game.gtimg.cn', 'wegame.gtimg.com', 'imgcache.qq.com', 'gw.alicdn.com', 'res.oppomobile.com', 'cdn.4399.com'],
    '新加坡': ['game.gtimg.cn', 'gw.alicdn.com', 'log.mmstat.com', 'sdkconfig.ad.xiaomi.com', 'img.client.10010.com', 'app.10086.cn'],
    '中国': ['game.gtimg.cn', 'wegame.gtimg.com', 'imgcache.qq.com', 'dlied1.qq.com', 'adash.m.taobao.com', 'gw.alicdn.com', 'sdkconfig.ad.xiaomi.com', 'img.client.10010.com', 'app.10086.cn', 'open.e.189.cn']
});

const COVER_FAMILY_PATTERNS = Object.freeze([
    /(^|\.)game\.gtimg\.cn$/i,
    /(^|\.)imgcache\.qq\.com$/i,
    /(^|\.)dlied\d*\.qq\.com$/i,
    /(^|\.)wegame\.gtimg\.com$/i,
    /(^|\.)pgdt\.gtimg\.cn$/i,
    /(^|\.)res\.wx\.qq\.com$/i,
    /(^|\.)qpic\.cn$/i,
    /(^|\.)gtimg\.cn$/i,
    /(^|\.)adash\.m\.taobao\.com$/i,
    /(^|\.)log\.mmstat\.com$/i,
    /(^|\.)gw\.alicdn\.com$/i,
    /(^|\.)img\.alicdn\.com$/i,
    /(^|\.)sdkconfig\.ad\.xiaomi\.com$/i,
    /(^|\.)tracking\.miui\.com$/i,
    /(^|\.)img\.client\.10010\.com$/i,
    /(^|\.)m\.client\.10010\.com$/i,
    /(^|\.)app\.10086\.cn$/i,
    /(^|\.)client\.10086\.cn$/i,
    /(^|\.)open\.e\.189\.cn$/i,
    /(^|\.)cloud\.189\.cn$/i,
    /(^|\.)res\.vivo\.com\.cn$/i,
    /(^|\.)res\.oppomobile\.com$/i,
    /(^|\.)api\.m\.mi\.com$/i,
    /(^|\.)webstatic\.mihoyo\.com$/i,
    /(^|\.)log-upload-os\.hoyoverse\.com$/i,
    /(^|\.)cdn\.4399\.com$/i,
    /(^|\.)steamstatic\.com$/i,
    /(^|\.)steamcontent\.com$/i
]);

const OBVIOUS_FOREIGN_PATTERNS = Object.freeze([
    /(^|\.)google(?:apis|usercontent|video)?\./i,
    /(^|\.)gstatic\.com$/i,
    /(^|\.)youtube\.com$/i,
    /(^|\.)cloudflare(?:-dns)?\./i,
    /(^|\.)cloudfront\.net$/i,
    /(^|\.)azure(edge|fd|websites)?\./i,
    /(^|\.)microsoft(?:online)?\.com$/i,
    /(^|\.)apple\.com$/i,
    /(^|\.)icloud\.com$/i,
    /(^|\.)fbcdn\.net$/i,
    /(^|\.)facebook\.com$/i,
    /(^|\.)instagram\.com$/i,
    /(^|\.)whatsapp\./i,
    /(^|\.)x\.com$/i,
    /(^|\.)twitter\.com$/i,
    /(^|\.)github(?:usercontent)?\.com$/i,
    /(^|\.)telegram\./i,
    /(^|\.)amazonaws\.com$/i,
    /(^|\.)fastly(?:lb)?\./i,
    /(^|\.)akamai(?:hd|zed)?\./i,
    /(^|\.)edge(key|suite)\./i
]);

const NOISY_PUBLIC_BRAND_PATTERNS = Object.freeze([
    /(^|\.)apple\.com(\.[a-z]{2,})?$/i,
    /(^|\.)icloud\.com(\.[a-z]{2,})?$/i,
    /(^|\.)biliimg\.com$/i,
    /(^|\.)hdslb\.com$/i,
    /(^|\.)bilibili\.com$/i,
    /(^|\.)steam(powered|community)?\.com$/i,
    /(^|\.)office\.com$/i,
    /(^|\.)office365\.com$/i
]);

const INFRA_HOST_PATTERNS = Object.freeze([
    /\.amazonaws\.com$/i,
    /\.googleusercontent\.com$/i,
    /\.cloudapp\./i,
    /\.azure/i,
    /\.digitalocean/i,
    /\.vultr/i,
    /\.linode/i
]);

const LOCAL_HOST_PATTERNS = Object.freeze([
    /^0\.0\.0\.0$/,
    /^127\./,
    /^10\./,
    /^192\.168\./,
    /^172\.(1[6-9]|2\d|3[01])\./,
    /(^|\.)localhost$/i,
    /\.local$/i
]);

const TLS_VERSION_RANK = Object.freeze({
    '1.0': 1,
    '1.1': 2,
    '1.2': 3,
    '1.3': 4
});

const COMMON_TRUSTED_TLD_PATTERN = /\.(?:com|net|org|cn|hk|jp|kr|sg|com\.cn|net\.cn|org\.cn|com\.hk|co\.jp|co\.kr|com\.sg)$/i;
const SUSPICIOUS_PROVIDER_TLD_PATTERN = /\.(?:top|xyz|vip|club|shop|site|online|lol|run|ink|bid|click|link)$/i;

const FORCE_OVERRIDE_POLICY = Object.freeze({
    strongSniScore: 90,
    acceptableSniScore: 35,
    serverOverrideMargin: 12,
    transportOverrideMargin: 20,
    minTransportHostScore: 0
});

const ALL_COVER_DOMAINS = (() => {
    const values = [];
    for (const list of Object.values(COVER_POOLS)) {
        for (const item of list) {
            values.push(item);
        }
    }
    return Object.freeze(new Set(values.map(v => v.toLowerCase())));
})();

const escapeRegex = (value) => String(value).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const BLOCK_REGEX = new RegExp(BLOCK_TERMS.map(escapeRegex).join('|'), 'i');

const stableHash = (input = '') => {
    let hash = 0;
    for (let i = 0; i < input.length; i++) {
        hash = ((hash << 5) - hash) + input.charCodeAt(i);
        hash |= 0;
    }
    return Math.abs(hash);
};

const pickStable = (items, seed = '') => {
    if (!Array.isArray(items) || items.length === 0) return '';
    return items[stableHash(seed || items[0]) % items.length];
};

const uniq = (items) => [...new Set(items)];

const cloneDeep = (value) => {
    if (typeof lodash !== 'undefined' && lodash && typeof lodash.cloneDeep === 'function') {
        return lodash.cloneDeep(value);
    }
    return JSON.parse(JSON.stringify(value));
};

const normalizeString = (value) => typeof value === 'string' ? value.trim() : '';

const get = (obj, path, defaultValue) => {
    if (!obj || typeof obj !== 'object') return defaultValue;
    const segments = Array.isArray(path) ? path : String(path).split('.');
    let current = obj;
    for (const segment of segments) {
        if (!current || typeof current !== 'object' || !(segment in current)) {
            return defaultValue;
        }
        current = current[segment];
    }
    return current;
};

const set = (obj, path, value) => {
    if (!obj || typeof obj !== 'object') return;
    const segments = Array.isArray(path) ? path : String(path).split('.');
    let current = obj;
    for (let i = 0; i < segments.length - 1; i++) {
        const key = segments[i];
        if (!current[key] || typeof current[key] !== 'object') {
            current[key] = {};
        }
        current = current[key];
    }
    current[segments[segments.length - 1]] = value;
};

const isIpv4Host = (host) => /^(\d{1,3}\.){3}\d{1,3}$/.test(host);

const isIpv6Host = (host) => {
    if (!host) return false;
    return host.includes(':') && /^[0-9a-f:]+$/i.test(host);
};

const isPrivateIpv4Host = (host) => {
    if (!isIpv4Host(host)) return false;
    const parts = host.split('.').map(v => parseInt(v, 10));
    if (parts.some(v => Number.isNaN(v) || v < 0 || v > 255)) return false;
    return parts[0] === 10 ||
        (parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31) ||
        (parts[0] === 192 && parts[1] === 168) ||
        parts[0] === 127 ||
        host === '0.0.0.0';
};

const isLiteralIp = (host) => isIpv4Host(host) || isIpv6Host(host);

const normalizeHost = (value) => {
    let host = normalizeString(value).toLowerCase();
    if (!host) return '';

    if (host.startsWith('[') && host.includes(']')) {
        host = host.slice(1, host.indexOf(']'));
    } else if ((host.match(/:/g) || []).length === 1 && /:\d+$/.test(host)) {
        host = host.replace(/:\d+$/, '');
    }

    if (host.includes('/')) {
        host = host.split('/')[0];
    }

    return host.replace(/\.$/, '');
};

const isProbablyDomain = (host) => {
    if (!host || isLiteralIp(host)) return false;
    if (!/^[a-z0-9.-]+$/i.test(host)) return false;
    if (host.includes('..')) return false;
    if (!host.includes('.')) return false;
    if (host.length > 253) return false;
    return host.split('.').every(part => !!part && part.length <= 63 && /^[a-z0-9-]+$/i.test(part) && !part.startsWith('-') && !part.endsWith('-'));
};

const matchAny = (value, patterns) => patterns.some(pattern => pattern.test(value));

const isUnsafeTlsHost = (host) => {
    if (!host) return true;
    if (LOCAL_HOST_PATTERNS.some(pattern => pattern.test(host))) return true;
    if (isPrivateIpv4Host(host)) return true;
    if (isLiteralIp(host)) return true;
    return !isProbablyDomain(host);
};

const normalizeArray = (value) => {
    if (Array.isArray(value)) {
        return value.map(v => normalizeString(v)).filter(Boolean);
    }
    if (typeof value === 'string') {
        return value.split(',').map(v => v.trim()).filter(Boolean);
    }
    return [];
};

const uniqueAlpn = (value) => uniq(normalizeArray(value).map(v => v.toLowerCase()));

const getPrimaryLabel = (host) => {
    const normalized = normalizeHost(host);
    if (!normalized || !normalized.includes('.')) return normalized;
    return normalized.split('.')[0];
};

const looksLikeAirportStyleHostname = (host) => {
    const label = getPrimaryLabel(host);
    if (!label) return false;
    return /[a-z]/i.test(label) && /\d/.test(label) && label.length >= 6;
};

const getOriginalName = (proxy, index) => normalizeString(proxy?.name) || `${String(proxy?.type || 'NODE').toUpperCase()} ${normalizeString(proxy?.server) || 'unknown'}:${proxy?.port || ''}` || `NODE ${index + 1}`;

const parsePort = (value) => {
    const port = parseInt(value, 10);
    return Number.isInteger(port) ? port : NaN;
};

const getRegionInfo = (name, server) => {
    const haystacks = [normalizeString(name), normalizeString(server)];
    for (const haystack of haystacks) {
        if (!haystack) continue;
        for (const info of REGION_PATTERNS) {
            if (info.pattern.test(haystack)) {
                return info;
            }
        }
    }

    const host = normalizeHost(server);
    const suffixMap = {
        '.hk': { flag: '🇭🇰', region: '香港', short: 'HK', priority: 10 },
        '.tw': { flag: '🇹🇼', region: '台湾', short: 'TW', priority: 11 },
        '.jp': { flag: '🇯🇵', region: '日本', short: 'JP', priority: 12 },
        '.kr': { flag: '🇰🇷', region: '韩国', short: 'KR', priority: 13 },
        '.sg': { flag: '🇸🇬', region: '新加坡', short: 'SG', priority: 14 },
        '.mo': { flag: '🇲🇴', region: '澳门', short: 'MO', priority: 15 },
        '.cn': { flag: '🇨🇳', region: '中国', short: 'CN', priority: 20 },
        '.us': { flag: '🇺🇸', region: '美国', short: 'US', priority: 30 },
        '.uk': { flag: '🇬🇧', region: '英国', short: 'UK', priority: 40 },
        '.de': { flag: '🇩🇪', region: '德国', short: 'DE', priority: 41 },
        '.fr': { flag: '🇫🇷', region: '法国', short: 'FR', priority: 42 },
        '.nl': { flag: '🇳🇱', region: '荷兰', short: 'NL', priority: 43 },
        '.au': { flag: '🇦🇺', region: '澳洲', short: 'AU', priority: 44 }
    };

    for (const [suffix, info] of Object.entries(suffixMap)) {
        if (host.endsWith(suffix)) return info;
    }

    return { flag: '🌐', region: '其他', short: 'XX', priority: 999 };
};

const getStableFingerprint = (regionName, seed) => {
    const pool = FINGERPRINT_POOLS[regionName] || FINGERPRINT_POOLS.default;
    return pickStable(pool, seed);
};

const sanitizeProxyMetadata = (proxy) => {
    if (!proxy || typeof proxy !== 'object') return;

    const headerPaths = ['ws-opts.headers', 'http-opts.headers', 'grpc-opts'];
    for (const path of headerPaths) {
        const headers = get(proxy, path);
        if (!headers || typeof headers !== 'object') continue;
        for (const key of Object.keys(headers)) {
            if (PROVIDER_TRACKING_HEADERS.has(key.toLowerCase())) {
                delete headers[key];
            }
        }
    }

    for (const key of TRACKING_FIELDS) {
        delete proxy[key];
    }
};

const stripManagedSwitchKeys = (proxy) => {
    for (const key of MANAGED_SWITCH_KEYS) {
        delete proxy[key];
    }
};

const finalizeSkipCertVerify = (proxy) => {
    if (!proxy || typeof proxy !== 'object') return;
    if (proxy._forceSkipCertVerify === true) {
        proxy['skip-cert-verify'] = true;
    } else {
        delete proxy['skip-cert-verify'];
    }
};

const hasTlsSurface = (proxy) => {
    return !!(
        proxy.tls === true ||
        proxy.sni ||
        proxy.servername ||
        proxy['server-name'] ||
        get(proxy, 'ws-opts.headers.Host') ||
        get(proxy, 'http-opts.headers.Host') ||
        get(proxy, 'h2-opts.host')
    );
};

const isRealityNode = (proxy) => !!(
    proxy?.['reality-opts'] ||
    proxy?.['reality-public-key'] ||
    proxy?.['public-key'] ||
    proxy?.pbk ||
    proxy?.publicKey ||
    proxy?.shortId ||
    proxy?.['short-id'] ||
    (proxy?.tls && proxy?.reality)
);

const hasXtlsFlow = (proxy) => {
    const flow = normalizeString(proxy?.flow).toLowerCase();
    return !!flow && (flow.includes('xtls') || flow.includes('vision') || flow.includes('splice') || flow.includes('direct'));
};

const isBlockedNodeName = (name) => {
    if (!name) return false;
    return BLOCK_REGEX.test(name);
};

const isInvalidServerHost = (server) => {
    const host = normalizeHost(server);
    if (!host) return true;
    if (matchAny(host, LOCAL_HOST_PATTERNS)) return true;
    if (host.includes('example.com') || host.includes('test.com')) return true;
    return false;
};

const shouldKeepProxy = (proxy) => {
    if (!proxy || typeof proxy !== 'object') return false;

    const type = normalizeString(proxy.type).toLowerCase();
    if (!PROTOCOL_WHITELIST.has(type)) return false;

    const port = parsePort(proxy.port);
    if (!normalizeString(proxy.server) || !Number.isInteger(port) || port <= 0 || port > 65535) return false;
    if (isInvalidServerHost(proxy.server)) return false;

    if (type === 'vless' && !normalizeString(proxy.uuid)) return false;
    if (type === 'hysteria2' && !normalizeString(proxy.password) && !normalizeString(proxy.auth) && !normalizeString(proxy['auth-str'])) return false;

    if (isBlockedNodeName(normalizeString(proxy.name))) return false;

    return true;
};

const getProxyUniqueKey = (proxy) => {
    const type = normalizeString(proxy.type).toLowerCase();
    const authKey = type === 'vless'
        ? normalizeString(proxy.uuid)
        : normalizeString(proxy.password || proxy.auth || proxy['auth-str']);
    const transportKey = [
        normalizeString(proxy.network).toLowerCase(),
        normalizeString(get(proxy, 'ws-opts.path')),
        normalizeString(get(proxy, 'grpc-opts.grpc-service-name') || proxy['grpc-service-name'] || proxy['service-name']),
        JSON.stringify(get(proxy, 'h2-opts.host') || ''),
        normalizeString(proxy.sni || proxy.servername || proxy['server-name'])
    ].join('|');

    return [
        type,
        normalizeHost(proxy.server),
        parsePort(proxy.port),
        authKey,
        transportKey
    ].join('|');
};

const appendCandidate = (list, seen, value, source, sourcePriority) => {
    const host = normalizeHost(value);
    if (!host || seen.has(`${source}:${host}`) || seen.has(`*:${host}`)) return;
    list.push({ host, source, sourcePriority });
    seen.add(`${source}:${host}`);
    seen.add(`*:${host}`);
};

const collectIdentityCandidates = (proxy) => {
    const candidates = [];
    const seen = new Set();

    appendCandidate(candidates, seen, proxy.sni, 'sni', 38);
    appendCandidate(candidates, seen, proxy.servername, 'servername', 37);
    appendCandidate(candidates, seen, proxy['server-name'], 'server-name', 37);
    appendCandidate(candidates, seen, get(proxy, 'ws-opts.headers.Host'), 'ws-host', 32);
    appendCandidate(candidates, seen, get(proxy, 'http-opts.headers.Host'), 'http-host', 31);
    appendCandidate(candidates, seen, get(proxy, 'obfs-opts.host'), 'obfs-host', 30);

    const h2Hosts = get(proxy, 'h2-opts.host');
    if (Array.isArray(h2Hosts)) {
        for (const host of h2Hosts) {
            appendCandidate(candidates, seen, host, 'h2-host', 33);
        }
    } else {
        appendCandidate(candidates, seen, h2Hosts, 'h2-host', 33);
    }

    appendCandidate(candidates, seen, proxy.server, 'server', 28);

    return candidates;
};

const collectTransportHostCandidates = (proxy) => {
    const candidates = [];
    const seen = new Set();

    appendCandidate(candidates, seen, get(proxy, 'ws-opts.headers.Host'), 'ws-host', 32);
    appendCandidate(candidates, seen, get(proxy, 'http-opts.headers.Host'), 'http-host', 31);
    appendCandidate(candidates, seen, get(proxy, 'obfs-opts.host'), 'obfs-host', 30);

    const h2Hosts = get(proxy, 'h2-opts.host');
    if (Array.isArray(h2Hosts)) {
        for (const host of h2Hosts) {
            appendCandidate(candidates, seen, host, 'h2-host', 33);
        }
    } else {
        appendCandidate(candidates, seen, h2Hosts, 'h2-host', 33);
    }

    return candidates;
};

const getStealthFallbackHost = (proxy, regionInfo) => {
    const regionPool = COVER_POOLS[regionInfo.region] || [];
    const defaultPool = COVER_POOLS.default || [];
    const pool = uniq([...regionPool, ...defaultPool].map(item => normalizeHost(item)).filter(Boolean));
    if (pool.length === 0) return '';
    const seed = `${normalizeHost(proxy.server)}:${proxy.port}:${proxy.name || ''}:${regionInfo.region}`;
    return pickStable(pool, seed);
};

const scoreCoverDomain = (domain, regionName, serverHost) => {
    if (!domain || isUnsafeTlsHost(domain)) {
        return { score: -999, level: 'invalid', reasons: ['无效域名'] };
    }

    const regionPool = COVER_POOLS[regionName] || [];
    let score = 0;
    const reasons = [];

    if (ALL_COVER_DOMAINS.has(domain)) {
        score += 70;
        reasons.push('命中伪装域白名单');
    }

    if (regionPool.includes(domain)) {
        score += 20;
        reasons.push('命中地区优选域');
    }

    if (matchAny(domain, COVER_FAMILY_PATTERNS)) {
        score += 40;
        reasons.push('命中游戏/运营商/采集域族');
    }

    if (serverHost && domain === serverHost && !isLiteralIp(serverHost)) {
        score += 24;
        reasons.push('与服务器域名一致');
    }

    if (COMMON_TRUSTED_TLD_PATTERN.test(domain)) {
        score += 10;
        reasons.push('常见证书站点后缀');
    }

    if (/^(www|api|cdn|img|res|ad|log|dl|client|app|open|service|update)\./i.test(domain)) {
        score += 6;
    }

    if (domain.split('.').length >= 3) {
        score += 4;
    }

    if (matchAny(domain, INFRA_HOST_PATTERNS)) {
        score -= 42;
        reasons.push('暴露基础设施');
    }

    if (SUSPICIOUS_PROVIDER_TLD_PATTERN.test(domain)) {
        score -= 18;
        reasons.push('机场常见廉价后缀');
    }

    if (matchAny(domain, OBVIOUS_FOREIGN_PATTERNS)) {
        score -= 55;
        reasons.push('明显境外大厂域名');
    }

    if (matchAny(domain, NOISY_PUBLIC_BRAND_PATTERNS)) {
        score -= 22;
        reasons.push('高暴露公共品牌域');
    }

    if (looksLikeAirportStyleHostname(domain)) {
        score -= 10;
        reasons.push('机场风格随机主机名');
    }

    let level = 'risky';
    if (score >= 90) level = 'strong';
    else if (score >= 35) level = 'usable';
    else if (score >= 0) level = 'weak';

    return { score, level, reasons };
};

const chooseBestCandidate = (candidates, regionInfo, serverHost) => {
    let bestAny = null;
    let bestNonNegative = null;

    for (const candidate of candidates) {
        const scored = scoreCoverDomain(candidate.host, regionInfo.region, serverHost);
        const total = scored.score + candidate.sourcePriority;
        const current = { ...candidate, ...scored, total };

        if (!bestAny || current.total > bestAny.total) {
            bestAny = current;
        }
        if (scored.score >= 0 && (!bestNonNegative || current.total > bestNonNegative.total)) {
            bestNonNegative = current;
        }
    }

    return bestNonNegative || bestAny || null;
};

const chooseTlsIdentityDecision = (proxy, regionInfo) => {
    const candidates = collectIdentityCandidates(proxy);
    const serverHost = normalizeHost(proxy.server);
    const currentExplicitHost = normalizeHost(proxy.sni || proxy.servername || proxy['server-name']);

    const explicitCandidates = candidates.filter(item =>
        item.source === 'sni' ||
        item.source === 'servername' ||
        item.source === 'server-name'
    );
    const bestExplicit = chooseBestCandidate(explicitCandidates, regionInfo, serverHost);

    const serverCandidates = candidates.filter(item => item.source === 'server');
    const bestServer = chooseBestCandidate(serverCandidates, regionInfo, serverHost);

    const transportCandidates = candidates.filter(item =>
        item.source === 'ws-host' ||
        item.source === 'http-host' ||
        item.source === 'h2-host' ||
        item.source === 'obfs-host'
    );
    const bestTransport = chooseBestCandidate(transportCandidates, regionInfo, serverHost);
    const fallbackHost = getStealthFallbackHost(proxy, regionInfo);
    const fallbackCandidate = fallbackHost
        ? chooseBestCandidate([{ host: fallbackHost, source: 'fallback-cover', sourcePriority: 29 }], regionInfo, serverHost)
        : null;

    if (bestExplicit && bestExplicit.score >= FORCE_OVERRIDE_POLICY.strongSniScore) {
        return {
            host: bestExplicit.host,
            source: bestExplicit.source,
            forceSkipCertVerify: false
        };
    }

    if (
        bestExplicit &&
        currentExplicitHost &&
        currentExplicitHost === serverHost &&
        bestExplicit.score >= FORCE_OVERRIDE_POLICY.acceptableSniScore
    ) {
        return {
            host: bestExplicit.host,
            source: bestExplicit.source,
            forceSkipCertVerify: false
        };
    }

    if (bestServer && bestServer.score > -999) {
        if (!bestExplicit && bestServer.score >= FORCE_OVERRIDE_POLICY.acceptableSniScore) {
            return {
                host: bestServer.host,
                source: bestServer.source,
                forceSkipCertVerify: false
            };
        }
        if (bestExplicit && bestExplicit.score < FORCE_OVERRIDE_POLICY.acceptableSniScore && bestServer.score >= FORCE_OVERRIDE_POLICY.acceptableSniScore) {
            return {
                host: bestServer.host,
                source: bestServer.source,
                forceSkipCertVerify: false
            };
        }
        if (bestServer.score >= FORCE_OVERRIDE_POLICY.acceptableSniScore && bestServer.score >= bestExplicit.score + FORCE_OVERRIDE_POLICY.serverOverrideMargin) {
            return {
                host: bestServer.host,
                source: bestServer.source,
                forceSkipCertVerify: false
            };
        }
    }

    if (
        bestTransport &&
        bestTransport.score >= FORCE_OVERRIDE_POLICY.acceptableSniScore &&
        (
            !bestServer ||
            bestServer.score < FORCE_OVERRIDE_POLICY.acceptableSniScore ||
            bestTransport.score >= bestServer.score + FORCE_OVERRIDE_POLICY.transportOverrideMargin
        )
    ) {
        return {
            host: bestTransport.host,
            source: bestTransport.source,
            forceSkipCertVerify: true
        };
    }

    if (
        fallbackCandidate &&
        fallbackCandidate.score >= FORCE_OVERRIDE_POLICY.acceptableSniScore &&
        (!bestExplicit || bestExplicit.score < FORCE_OVERRIDE_POLICY.acceptableSniScore) &&
        (!bestServer || bestServer.score < FORCE_OVERRIDE_POLICY.acceptableSniScore)
    ) {
        return {
            host: fallbackCandidate.host,
            source: fallbackCandidate.source,
            forceSkipCertVerify: true
        };
    }

    if (bestExplicit && bestExplicit.score > -999) {
        return {
            host: bestExplicit.host,
            source: bestExplicit.source,
            forceSkipCertVerify: false
        };
    }

    if (bestServer && bestServer.score > -999) {
        return {
            host: bestServer.host,
            source: bestServer.source,
            forceSkipCertVerify: false
        };
    }

    if (bestTransport && bestTransport.score > -999) {
        return {
            host: bestTransport.host,
            source: bestTransport.source,
            forceSkipCertVerify: true
        };
    }

    if (fallbackCandidate && fallbackCandidate.score > -999) {
        return {
            host: fallbackCandidate.host,
            source: fallbackCandidate.source,
            forceSkipCertVerify: true
        };
    }

    return {
        host: '',
        source: '',
        forceSkipCertVerify: false
    };
};

const chooseForcedTransportHost = (proxy, regionInfo, authoritativeSni) => {
    const serverHost = normalizeHost(proxy.server);
    const bestTransport = chooseBestCandidate(collectTransportHostCandidates(proxy), regionInfo, serverHost);
    const authoritativeScore = scoreCoverDomain(authoritativeSni, regionInfo.region, serverHost);

    if (bestTransport && bestTransport.score >= FORCE_OVERRIDE_POLICY.minTransportHostScore) {
        if (!authoritativeSni) return bestTransport.host;
        if (authoritativeScore.score < FORCE_OVERRIDE_POLICY.minTransportHostScore) return bestTransport.host;
        if (bestTransport.score >= authoritativeScore.score + FORCE_OVERRIDE_POLICY.transportOverrideMargin) {
            return bestTransport.host;
        }
        return bestTransport.host;
    }

    if (authoritativeSni && authoritativeScore.score >= FORCE_OVERRIDE_POLICY.minTransportHostScore) {
        return authoritativeSni;
    }

    return '';
};

const shouldForceTransportHostForNetwork = (proxy, fieldValue, networkType) => {
    const network = normalizeString(proxy.network).toLowerCase();
    if (fieldValue !== undefined) return true;
    return network === networkType;
};

const applyUnifiedTlsIdentity = (proxy, identityHost, regionInfo) => {
    if (!identityHost) return;

    proxy.sni = identityHost;
    if (proxy.servername !== undefined) proxy.servername = identityHost;
    if (proxy['server-name'] !== undefined) proxy['server-name'] = identityHost;

    const serverHost = normalizeHost(proxy.server);
    const existingWsHost = get(proxy, 'ws-opts.headers.Host');
    const existingHttpHost = get(proxy, 'http-opts.headers.Host');
    const existingH2Host = get(proxy, 'h2-opts.host');
    const existingObfsHost = get(proxy, 'obfs-opts.host');
    const forcedTransportHost = chooseForcedTransportHost(proxy, regionInfo, identityHost) || identityHost;
    const forcedTransportScore = scoreCoverDomain(forcedTransportHost, regionInfo.region, serverHost).score;

    if (forcedTransportScore < FORCE_OVERRIDE_POLICY.minTransportHostScore) return;

    if (shouldForceTransportHostForNetwork(proxy, existingWsHost, 'ws')) {
        set(proxy, 'ws-opts.headers.Host', forcedTransportHost);
    }
    if (shouldForceTransportHostForNetwork(proxy, existingHttpHost, 'http')) {
        set(proxy, 'http-opts.headers.Host', forcedTransportHost);
    }
    if (shouldForceTransportHostForNetwork(proxy, existingH2Host, 'h2')) {
        set(proxy, 'h2-opts.host', [forcedTransportHost]);
    }
    if (existingObfsHost !== undefined) {
        set(proxy, 'obfs-opts.host', forcedTransportHost);
    }
};

const normalizeTlsVersionRange = (proxy, defaultMin, defaultMax, force = false) => {
    const currentMin = normalizeString(proxy['tls-min-version']);
    const currentMax = normalizeString(proxy['tls-max-version']);

    if (force) {
        proxy['tls-min-version'] = defaultMin;
        proxy['tls-max-version'] = defaultMax;
        return;
    }

    const nextMin = TLS_VERSION_RANK[currentMin] ? currentMin : defaultMin;
    const nextMax = TLS_VERSION_RANK[currentMax] ? currentMax : defaultMax;

    proxy['tls-min-version'] = TLS_VERSION_RANK[nextMin] < TLS_VERSION_RANK[defaultMin] ? defaultMin : nextMin;
    proxy['tls-max-version'] = TLS_VERSION_RANK[nextMax] > TLS_VERSION_RANK[defaultMax] ? defaultMax : nextMax;

    if (TLS_VERSION_RANK[proxy['tls-min-version']] > TLS_VERSION_RANK[proxy['tls-max-version']]) {
        proxy['tls-min-version'] = defaultMin;
        proxy['tls-max-version'] = defaultMax;
    }
};

const normalizeAlpn = (proxy, preferred, hard = false) => {
    const normalizedPreferred = uniq(preferred.map(v => v.toLowerCase()));
    if (hard) {
        proxy.alpn = normalizedPreferred;
        return;
    }

    const merged = uniq([...normalizedPreferred, ...uniqueAlpn(proxy.alpn)]);
    proxy.alpn = merged.length > 0 ? merged : normalizedPreferred;
};

const applyStableFingerprint = (proxy, regionInfo) => {
    const seed = `${normalizeHost(proxy.server)}:${proxy.port}:${proxy.type}:${proxy.name || ''}`;
    proxy['client-fingerprint'] = getStableFingerprint(regionInfo.region, seed);
};

const getTransportTag = (proxy) => {
    const type = normalizeString(proxy.type).toLowerCase();
    if (type === 'hysteria2') return 'HY2';

    const network = normalizeString(proxy.network).toLowerCase();
    if (isRealityNode(proxy)) return 'R';
    if (network === 'ws') return 'WS';
    if (network === 'grpc') return 'gRPC';
    if (network === 'h2') return 'H2';
    if (network === 'quic') return 'QUIC';
    return 'TCP';
};

const getLineTag = (name) => {
    const text = normalizeString(name);
    if (!text) return '';
    if (/iplc|iepl|专线/i.test(text)) return 'IPLC';
    if (/cn2|gia/i.test(text)) return 'CN2';
    if (/9929|4837|cmi/i.test(text)) return '骨干';
    if (/game|gaming|游戏/i.test(text)) return 'GAME';
    if (/chatgpt|openai|gpt|ai/i.test(text)) return 'AI';
    if (/netflix|nf|disney|emby|流媒体|解锁/i.test(text)) return 'MEDIA';
    return '';
};

const buildNodeName = (proxy, regionInfo, index, originalName) => {
    const padded = String(index).padStart(2, '0');
    const tags = [getTransportTag(proxy), getLineTag(originalName)].filter(Boolean);
    return `${regionInfo.flag} ${regionInfo.short}·${padded}${tags.length ? ` ${tags.join('·')}` : ''}`.trim();
};

const optimizeVless = (proxy, regionInfo) => {
    proxy['ip-version'] = 'prefer-v6';
    delete proxy._forceSkipCertVerify;

    const reality = isRealityNode(proxy);
    const xtls = hasXtlsFlow(proxy);
    const tlsLike = reality || proxy.tls === true || hasTlsSurface(proxy);
    if (!tlsLike) return;

    if (!reality && proxy.tls !== true) {
        return;
    }

    if (!reality) {
        proxy.tls = true;
        normalizeTlsVersionRange(proxy, '1.2', '1.3');
    }

    applyStableFingerprint(proxy, regionInfo);

    const network = normalizeString(proxy.network).toLowerCase();
    if (!reality) {
        if (network === 'grpc' || network === 'h2') {
            normalizeAlpn(proxy, ['h2'], true);
        } else if (network === 'quic') {
            normalizeAlpn(proxy, ['h3'], true);
        } else {
            normalizeAlpn(proxy, ['h2', 'http/1.1']);
        }
    }

    if (reality || xtls) {
        return;
    }

    const decision = chooseTlsIdentityDecision(proxy, regionInfo);
    if (!decision || !decision.host) return;
    if (decision.forceSkipCertVerify) {
        proxy._forceSkipCertVerify = true;
    }
    applyUnifiedTlsIdentity(proxy, decision.host, regionInfo);
};

const optimizeHysteria2 = (proxy, regionInfo) => {
    proxy['ip-version'] = 'prefer-v6';
    delete proxy._forceSkipCertVerify;
    proxy.tls = true;
    normalizeTlsVersionRange(proxy, '1.3', '1.3', true);
    normalizeAlpn(proxy, ['h3'], true);
    applyStableFingerprint(proxy, regionInfo);

    const decision = chooseTlsIdentityDecision(proxy, regionInfo);
    if (!decision || !decision.host) {
        const serverHost = normalizeHost(proxy.server);
        if (isProbablyDomain(serverHost) && !isUnsafeTlsHost(serverHost)) {
            proxy.sni = serverHost;
        }
        return;
    }

    if (decision.forceSkipCertVerify) {
        proxy._forceSkipCertVerify = true;
    }

    proxy.sni = decision.host;
    if (proxy.servername !== undefined) proxy.servername = decision.host;
    if (proxy['server-name'] !== undefined) proxy['server-name'] = decision.host;
};

const optimizeProxy = (proxy, regionInfo) => {
    const type = normalizeString(proxy.type).toLowerCase();

    if (type === 'vless') {
        optimizeVless(proxy, regionInfo);
    } else if (type === 'hysteria2') {
        optimizeHysteria2(proxy, regionInfo);
    }

    return proxy;
};

async function operator(proxies = []) {
    try {
        if (!Array.isArray(proxies) || proxies.length === 0) {
            return [];
        }

        const deduped = [];
        const seen = new Set();

        for (let i = 0; i < proxies.length; i++) {
            const proxy = proxies[i];
            if (!shouldKeepProxy(proxy)) continue;

            const key = getProxyUniqueKey(proxy);
            if (seen.has(key)) continue;
            seen.add(key);

            const cloned = cloneDeep(proxy);
            cloned._sourceIndex = i;
            cloned._originalName = getOriginalName(proxy, i);
            deduped.push(cloned);
        }

        const regionCounters = new Map();
        const finalNodes = [];

        for (const proxy of deduped) {
            try {
                const regionInfo = getRegionInfo(proxy._originalName, proxy.server);
                proxy._regionPriority = regionInfo.priority;
                proxy._regionName = regionInfo.region;
                proxy._regionFlag = regionInfo.flag;

                optimizeProxy(proxy, regionInfo);
                sanitizeProxyMetadata(proxy);
                stripManagedSwitchKeys(proxy);
                finalizeSkipCertVerify(proxy);

                const nextCount = (regionCounters.get(regionInfo.region) || 0) + 1;
                regionCounters.set(regionInfo.region, nextCount);
                proxy.name = buildNodeName(proxy, regionInfo, nextCount, proxy._originalName);

                finalNodes.push(proxy);
            } catch (error) {
                console.log(`[rules_entrance] 节点处理失败，已跳过: ${error.message}`);
            }
        }

        finalNodes.sort((a, b) => {
            const pA = a._regionPriority ?? 999;
            const pB = b._regionPriority ?? 999;
            if (pA !== pB) return pA - pB;
            return (a._sourceIndex ?? 0) - (b._sourceIndex ?? 0);
        });

        for (const node of finalNodes) {
            for (const key of TEMP_KEYS) {
                delete node[key];
            }
        }

        console.log(`[rules_entrance] 完成: 输入 ${proxies.length} -> 输出 ${finalNodes.length}`);
        return finalNodes;
    } catch (error) {
        console.log(`[rules_entrance] 失败: ${error.message}`);
        return proxies;
    }
}
