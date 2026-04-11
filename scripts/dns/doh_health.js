/**
 * DoH Health Monitor - Encrypted DNS Server Availability Check
 *
 * Periodically tests DoH server latency and availability via cron.
 * Notifies when servers are unreachable or degraded.
 *
 * Supported: Surge (5.x+), Shadowrocket (2.x+)
 * Type: cron script
 *
 * @author nyamiiko
 * @version 2026.04.12
 */

const NAME = "DoH Monitor";
const VERSION = "2026.04.12";

// DoH servers to monitor - same order as module config
const DOH_SERVERS = [
  {
    name: "AliDNS",
    url: "https://dns.alidns.com/dns-query?dns=AAABAAABAAAAAAAAA3d3dwZnb29nbGUDY29tAAABAAE",
    region: "CN",
  },
  {
    name: "Cloudflare",
    url: "https://cloudflare-dns.com/dns-query?dns=AAABAAABAAAAAAAAA3d3dwZnb29nbGUDY29tAAABAAE",
    region: "Global",
  },
  {
    name: "Quad9",
    url: "https://dns11.quad9.net/dns-query?dns=AAABAAABAAAAAAAAA3d3dwZnb29nbGUDY29tAAABAAE",
    region: "Global",
  },
  {
    name: "AdGuard",
    url: "https://dns.adguard-dns.com/dns-query?dns=AAABAAABAAAAAAAAA3d3dwZnb29nbGUDY29tAAABAAE",
    region: "Global",
  },
  {
    name: "Google",
    url: "https://dns.google/dns-query?dns=AAABAAABAAAAAAAAA3d3dwZnb29nbGUDY29tAAABAAE",
    region: "Global",
  },
  {
    name: "DNSPod",
    url: "https://doh.pub/dns-query?dns=AAABAAABAAAAAAAAA3d3dwZnb29nbGUDY29tAAABAAE",
    region: "CN",
  },
  {
    name: "360DNS",
    url: "https://doh.360.cn/dns-query?dns=AAABAAABAAAAAAAAA3d3dwZnb29nbGUDY29tAAABAAE",
    region: "CN",
  },
];

const TIMEOUT = 5000; // 5s per server
const SLOW_THRESHOLD = 2000; // >2s = degraded

/**
 * Notification helper
 */
function notify(title, subtitle, body) {
  if (typeof $notification !== "undefined" && $notification) {
    $notification.post(title, subtitle, body);
  }
}

/**
 * Store helpers
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

/**
 * Test a single DoH server
 * Returns: { name, region, status, latency, error? }
 */
function testServer(server) {
  return new Promise((resolve) => {
    const start = Date.now();
    const timer = setTimeout(() => {
      resolve({
        name: server.name,
        region: server.region,
        status: "timeout",
        latency: TIMEOUT,
      });
    }, TIMEOUT);

    $httpClient.get(
      {
        url: server.url,
        timeout: TIMEOUT / 1000,
        headers: {
          Accept: "application/dns-message",
          "User-Agent": `DoH-Monitor/${VERSION}`,
        },
      },
      (error, response, _data) => {
        clearTimeout(timer);
        const latency = Date.now() - start;
        if (error) {
          resolve({
            name: server.name,
            region: server.region,
            status: "error",
            latency,
            error: String(error).substring(0, 80),
          });
        } else if (response && response.status >= 200 && response.status < 400) {
          resolve({
            name: server.name,
            region: server.region,
            status: latency > SLOW_THRESHOLD ? "slow" : "ok",
            latency,
          });
        } else {
          resolve({
            name: server.name,
            region: server.region,
            status: "error",
            latency,
            error: `HTTP ${response ? response.status : "?"}`,
          });
        }
      }
    );
  });
}

/**
 * Main health check
 */
async function main() {
  const results = await Promise.all(DOH_SERVERS.map(testServer));

  const ok = results.filter((r) => r.status === "ok");
  const slow = results.filter((r) => r.status === "slow");
  const down = results.filter(
    (r) => r.status === "error" || r.status === "timeout"
  );

  // Build status summary
  const lines = results.map((r) => {
    const icon =
      r.status === "ok" ? "✅" : r.status === "slow" ? "⚠️" : "❌";
    return `${icon} ${r.name} (${r.region}): ${r.latency}ms`;
  });

  // Store last check time and results
  setStore("doh_last_check", new Date().toISOString());
  setStore(
    "doh_status",
    JSON.stringify({
      ok: ok.length,
      slow: slow.length,
      down: down.length,
      fastest: ok.length > 0 ? ok.sort((a, b) => a.latency - b.latency)[0].name : "N/A",
    })
  );

  // Always notify if any server is down; silent if all healthy
  if (down.length > 0) {
    notify(
      `${NAME} - ${down.length} Server(s) Down`,
      `${ok.length}✅ ${slow.length}⚠️ ${down.length}❌`,
      lines.join("\n")
    );
  } else if (slow.length > 0) {
    // Only notify for slow if all were healthy last time
    const lastStatus = getStore("doh_last_status", "ok");
    if (lastStatus === "ok") {
      notify(
        `${NAME} - ${slow.length} Server(s) Slow`,
        `${ok.length}✅ ${slow.length}⚠️`,
        lines.join("\n")
      );
    }
  }

  setStore("doh_last_status", down.length > 0 ? "down" : slow.length > 0 ? "slow" : "ok");

  $done();
}

main();
