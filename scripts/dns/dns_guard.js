/**
 * DNS Guard - HTTPDNS Interception & DNS Poisoning Protection
 *
 * Intercepts HTTPDNS requests by URL pattern and validates DNS responses
 * to prevent DNS hijacking/poisoning in apps that bypass system DNS.
 *
 * Supported: Surge (5.x+), Shadowrocket (2.x+)
 * Type: http-request / http-response
 *
 * @author nyamiiko
 * @version 2026.04.12
 */

const NAME = "DNS Guard";
const VERSION = "2026.04.12";

// Known bogon / poisoned IP ranges - these should never appear in DNS responses
const BOGON_CIDRS = [
  // RFC 1918 private
  { prefix: "10.", mask: 8 },
  { prefix: "172.16.", mask: 12 },
  { prefix: "172.17.", mask: 12 },
  { prefix: "172.18.", mask: 12 },
  { prefix: "172.19.", mask: 12 },
  { prefix: "172.20.", mask: 12 },
  { prefix: "172.21.", mask: 12 },
  { prefix: "172.22.", mask: 12 },
  { prefix: "172.23.", mask: 12 },
  { prefix: "172.24.", mask: 12 },
  { prefix: "172.25.", mask: 12 },
  { prefix: "172.26.", mask: 12 },
  { prefix: "172.27.", mask: 12 },
  { prefix: "172.28.", mask: 12 },
  { prefix: "172.29.", mask: 12 },
  { prefix: "172.30.", mask: 12 },
  { prefix: "172.31.", mask: 12 },
  { prefix: "192.168.", mask: 16 },
  // Loopback
  { prefix: "127.", mask: 8 },
  // Link-local
  { prefix: "169.254.", mask: 16 },
  // Known GFW poisoned IPs (commonly injected)
  { prefix: "93.46.8.89", exact: true },
  { prefix: "37.61.54.158", exact: true },
  { prefix: "243.185.187.39", exact: true },
  { prefix: "46.82.174.68", exact: true },
  { prefix: "78.16.49.15", exact: true },
  { prefix: "59.24.3.173", exact: true },
  { prefix: "203.98.7.65", exact: true },
  { prefix: "8.7.198.45", exact: true },
  { prefix: "159.106.121.75", exact: true },
  { prefix: "2.1.1.2", exact: true },
];

// HTTPDNS URL patterns to intercept
const HTTPDNS_PATTERNS = [
  // Alibaba/Aliyun HTTPDNS
  /httpdns[.-].*\.ali(cdn|yuncs|dns)/i,
  /203\.107\.1\.\d+/,
  // Baidu HTTPDNS
  /https?dns\.baidu(bce)?\.com/i,
  /180\.76\.76\.200/,
  // Tencent HTTPDNS
  /119\.29\.29\.(98|99)/,
  /httpdns\.pro/i,
  // ByteDance/Volcengine HTTPDNS
  /httpdns\.volcengine/i,
  // Bilibili HTTPDNS
  /httpdns\.bilivideo\.com/i,
  // Huawei HTTPDNS
  /httpdns.*\.cdnhwc/i,
  // JD HTTPDNS
  /dns\.jd\.com/i,
  /101\.124\.19\.122/,
  // Meituan HTTPDNS
  /httpdns(vip)?\.meituan\.com/i,
  // NetEase HTTPDNS
  /httpdns\.(yunxindns|n\.netease|music\.163)/i,
  /music\.httpdns/i,
  /lofter\.httpdns/i,
  /59\.111\.239\.(61|62)/,
  // Weibo HTTPDNS
  /dns\.weibo\.cn/i,
  // WeChat HTTPDNS
  /dns\.weixin\.qq\.com/i,
  // Oppo HTTPDNS
  /httpdns\.push\.oppomobile/i,
  // Kuaishou/Keep HTTPDNS
  /httpdns\.calorietech/i,
  // Generic HTTPDNS pattern
  /\/d\?host=/i,
  /\/resolve\?name=/i,
  /\/httpdns\//i,
  /dns-query.*application\/dns/i,
];

/**
 * Check if an IP is a bogon or known-poisoned address
 */
function isBogonOrPoisoned(ip) {
  if (!ip || typeof ip !== "string") return false;
  for (const entry of BOGON_CIDRS) {
    if (entry.exact) {
      if (ip === entry.prefix) return true;
    } else {
      if (ip.startsWith(entry.prefix)) return true;
    }
  }
  return false;
}

/**
 * Check if URL matches known HTTPDNS patterns
 */
function isHTTPDNS(url) {
  if (!url) return false;
  for (const pattern of HTTPDNS_PATTERNS) {
    if (pattern.test(url)) return true;
  }
  return false;
}

/**
 * Validate DNS response body for poisoned IPs
 * Returns { clean: bool, poisoned: string[] }
 */
function validateDNSResponse(body) {
  if (!body) return { clean: true, poisoned: [] };

  const poisoned = [];
  // Match IPv4 addresses in response
  const ipv4Regex = /\b(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})\b/g;
  let match;
  while ((match = ipv4Regex.exec(body)) !== null) {
    if (isBogonOrPoisoned(match[1])) {
      poisoned.push(match[1]);
    }
  }
  return { clean: poisoned.length === 0, poisoned };
}

/**
 * Get notification function for current environment
 */
function notify(title, subtitle, body) {
  const env = typeof $notification !== "undefined" ? "surge" : "unknown";
  if (env === "surge" && $notification) {
    $notification.post(title, subtitle, body);
  }
}

/**
 * Persistent store helpers
 */
function getStore(key, fallback) {
  if (typeof $persistentStore !== "undefined") {
    const val = $persistentStore.read(key);
    return val !== null && val !== undefined ? val : fallback;
  }
  return fallback;
}

function setStore(key, val) {
  if (typeof $persistentStore !== "undefined") {
    $persistentStore.write(String(val), key);
  }
}

// ========== Main Logic ==========

(function main() {
  const url = typeof $request !== "undefined" ? $request.url : "";
  const isRequest = typeof $response === "undefined";

  // --- Phase 1: HTTP-Request - Block HTTPDNS requests ---
  if (isRequest) {
    if (isHTTPDNS(url)) {
      // Increment block counter
      const count = parseInt(getStore("dns_guard_block_count", "0"), 10) + 1;
      setStore("dns_guard_block_count", count);

      // Rate-limited notifications (every 50 blocks)
      if (count % 50 === 1) {
        notify(
          `${NAME} v${VERSION}`,
          `Blocked HTTPDNS (${count} total)`,
          `URL: ${url.substring(0, 120)}`
        );
      }

      // Reject the request with empty body
      $done({ response: { status: 403, body: "" } });
      return;
    }
    $done({});
    return;
  }

  // --- Phase 2: HTTP-Response - Validate DNS responses ---
  if (!isRequest && $response) {
    const body = $response.body || "";
    const contentType = ($response.headers || {})["Content-Type"] || "";

    // Check if this looks like a DNS response
    const isDNSLike =
      contentType.includes("dns") ||
      contentType.includes("json") ||
      /resolve|dns-query|dns\./.test(url);

    if (isDNSLike) {
      const result = validateDNSResponse(body);
      if (!result.clean) {
        const poisonCount =
          parseInt(getStore("dns_guard_poison_count", "0"), 10) + 1;
        setStore("dns_guard_poison_count", poisonCount);

        notify(
          `${NAME} - DNS Poisoning Detected!`,
          `${result.poisoned.length} bogus IPs found`,
          `IPs: ${result.poisoned.join(", ")}\nURL: ${url.substring(0, 100)}`
        );

        // Strip poisoned IPs from response if it's JSON
        if (contentType.includes("json")) {
          let cleaned = body;
          for (const ip of result.poisoned) {
            cleaned = cleaned.split(ip).join("0.0.0.0");
          }
          $done({ body: cleaned });
          return;
        }

        // For non-JSON, block entirely
        $done({ response: { status: 502, body: "" } });
        return;
      }
    }

    $done({});
    return;
  }

  $done({});
})();
