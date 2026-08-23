use std::collections::{BTreeSet, HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USER_AGENT: &str = "ScriptHub-PROMAX/1.0";
const MIN_DOWNLOAD_CONCURRENCY: usize = 1;
const MAX_DOWNLOAD_CONCURRENCY: usize = 6;
const GITHUB_REQUEST_INTERVAL: Duration = Duration::from_millis(125);
// Hard safety ceiling for malformed server headers; normal Retry-After values
// are otherwise honored in full and the batch deadline stops the current run.
const MAX_RATE_LIMIT_DELAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub attempts: usize,
    pub timeout: Duration,
    pub batch_timeout: Duration,
    pub backoff: Duration,
    pub concurrency: usize,
    pub max_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            attempts: 3,
            timeout: Duration::from_secs(60),
            batch_timeout: Duration::from_secs(10 * 60),
            backoff: Duration::from_secs(1),
            concurrency: 4,
            max_bytes: 64 * 1024 * 1024,
            max_total_bytes: 256 * 1024 * 1024,
        }
    }
}

pub struct Downloader {
    agent: ureq::Agent,
    config: DownloadConfig,
    github_not_before: Mutex<Option<Instant>>,
}

#[derive(Debug, Default)]
pub struct DownloadBatch {
    pub contents: HashMap<String, String>,
    pub failures: Vec<DownloadFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadFailure {
    pub url: String,
    pub error: String,
}

impl Downloader {
    pub fn new(config: DownloadConfig) -> Self {
        let agent_config = ureq::Agent::config_builder()
            .timeout_global(Some(config.timeout))
            .max_redirects(5)
            .http_status_as_error(false)
            .user_agent(USER_AGENT)
            .proxy(configured_proxy())
            .build();
        Self {
            agent: agent_config.into(),
            config,
            github_not_before: Mutex::new(None),
        }
    }

    pub fn get(&self, url: &str) -> Result<String, DownloadError> {
        self.get_until(url, None)
    }

    fn get_until(&self, url: &str, deadline: Option<Instant>) -> Result<String, DownloadError> {
        let attempts = self.config.attempts.max(1);
        let candidates = download_candidates(url);
        let mut last_error = None;
        for attempt in 1..=attempts {
            let candidate = &candidates[(attempt - 1) % candidates.len()];
            if !self.wait_for_github_slot(candidate, deadline) {
                last_error = Some("batch deadline exceeded".to_string());
                break;
            }
            let timeout = deadline
                .map(|deadline| deadline.saturating_duration_since(Instant::now()))
                .map(|remaining| remaining.min(self.config.timeout))
                .unwrap_or(self.config.timeout);
            if timeout.is_zero() {
                last_error = Some("batch deadline exceeded".to_string());
                break;
            }
            let mut delay_override = None;
            let mut retryable = true;
            match self
                .agent
                .get(candidate)
                .config()
                .timeout_global(Some(timeout))
                .build()
                .call()
            {
                Ok(mut response) if response.status().is_success() => {
                    match response
                        .body_mut()
                        .with_config()
                        .limit(self.config.max_bytes)
                        .read_to_string()
                    {
                        Ok(body) => return Ok(body),
                        Err(error) => last_error = Some(error.to_string()),
                    }
                }
                Ok(response) => {
                    let status = response.status().as_u16();
                    retryable = status == 403 || status == 429 || status >= 500;
                    if status == 403 || status == 429 {
                        let delay = response
                            .headers()
                            .get("retry-after")
                            .and_then(|value| value.to_str().ok())
                            .and_then(|value| value.parse::<u64>().ok())
                            .map(Duration::from_secs)
                            .or_else(|| {
                                let reset = response
                                    .headers()
                                    .get("x-ratelimit-reset")?
                                    .to_str()
                                    .ok()?
                                    .parse::<u64>()
                                    .ok()?;
                                let now =
                                    SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
                                Some(Duration::from_secs(reset.saturating_sub(now)))
                            })
                            .unwrap_or(MAX_RATE_LIMIT_DELAY)
                            .min(MAX_RATE_LIMIT_DELAY);
                        if is_github_url(candidate) {
                            self.defer_github(delay);
                            delay_override = Some(Duration::ZERO);
                        } else {
                            delay_override = Some(delay);
                        }
                    }
                    last_error = Some(format!("http status {status}"));
                }
                Err(error) => last_error = Some(format!("{candidate}: {error}")),
            }

            if !retryable {
                break;
            }
            if attempt < attempts {
                let delay = delay_override
                    .unwrap_or_else(|| self.config.backoff.saturating_mul(attempt as u32));
                let delay = deadline
                    .map(|deadline| delay.min(deadline.saturating_duration_since(Instant::now())))
                    .unwrap_or(delay);
                if !delay.is_zero() {
                    thread::sleep(delay);
                }
            }
        }

        Err(DownloadError(format!(
            "failed to download {url} after {attempts} attempt(s): {}",
            last_error.unwrap_or_else(|| "unknown download error".to_string())
        )))
    }

    pub fn download_many<I, S>(&self, urls: I) -> DownloadBatch
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let urls: BTreeSet<String> = urls
            .into_iter()
            .map(|url| url.as_ref().to_string())
            .collect();
        let mut requested_concurrency = std::env::var("SCRIPTHUB_DOWNLOAD_CONCURRENCY")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(self.config.concurrency);
        requested_concurrency = bounded_concurrency(requested_concurrency, urls.len());

        let queue = Mutex::new(VecDeque::from_iter(urls));
        let results = Mutex::new(Vec::new());
        let remaining_bytes = Mutex::new(self.config.max_total_bytes);
        let deadline = Instant::now() + self.config.batch_timeout;
        thread::scope(|scope| {
            for _ in 0..requested_concurrency {
                scope.spawn(|| {
                    loop {
                        let Some(url) = queue.lock().expect("download queue poisoned").pop_front()
                        else {
                            break;
                        };
                        if Instant::now() >= deadline {
                            break;
                        }
                        let result = self.get_until(&url, Some(deadline)).and_then(|content| {
                            let mut remaining = remaining_bytes
                                .lock()
                                .expect("download byte budget poisoned");
                            let size = content.len() as u64;
                            if size > *remaining {
                                Err(DownloadError(format!(
                                    "download batch exceeds {} bytes",
                                    self.config.max_total_bytes
                                )))
                            } else {
                                *remaining -= size;
                                Ok(content)
                            }
                        });
                        results
                            .lock()
                            .expect("download results poisoned")
                            .push((url, result));
                    }
                });
            }
        });

        let mut results = results.into_inner().expect("download results poisoned");
        results.extend(
            queue
                .into_inner()
                .expect("download queue poisoned")
                .into_iter()
                .map(|url| {
                    (
                        url,
                        Err(DownloadError(
                            "download batch deadline exceeded".to_string(),
                        )),
                    )
                }),
        );
        results.sort_by(|left, right| left.0.cmp(&right.0));
        let mut batch = DownloadBatch::default();
        for (url, result) in results {
            match result {
                Ok(content) => {
                    batch.contents.insert(url, content);
                }
                Err(error) => batch.failures.push(DownloadFailure {
                    url,
                    error: error.to_string(),
                }),
            }
        }
        batch
    }

    fn wait_for_github_slot(&self, url: &str, deadline: Option<Instant>) -> bool {
        if !is_github_url(url) {
            return deadline.is_none_or(|deadline| Instant::now() < deadline);
        }
        loop {
            let wait = {
                let mut not_before = self
                    .github_not_before
                    .lock()
                    .expect("GitHub rate gate poisoned");
                let now = Instant::now();
                if deadline.is_some_and(|deadline| now >= deadline) {
                    return false;
                }
                match *not_before {
                    Some(next) if next > now => next.duration_since(now),
                    _ => {
                        *not_before = Some(now + GITHUB_REQUEST_INTERVAL);
                        return true;
                    }
                }
            };
            let wait = deadline
                .map(|deadline| wait.min(deadline.saturating_duration_since(Instant::now())))
                .unwrap_or(wait);
            if wait.is_zero() {
                return false;
            }
            thread::sleep(wait);
        }
    }

    fn defer_github(&self, delay: Duration) {
        let target = Instant::now() + delay.min(MAX_RATE_LIMIT_DELAY);
        let mut not_before = self
            .github_not_before
            .lock()
            .expect("GitHub rate gate poisoned");
        if not_before.is_none_or(|current| current < target) {
            *not_before = Some(target);
        }
    }
}

fn configured_proxy() -> Option<ureq::Proxy> {
    let value = |names: &[&str]| names.iter().find_map(|name| std::env::var(name).ok());
    proxy_from_values(
        value(&["HTTPS_PROXY", "https_proxy"]).as_deref(),
        value(&["HTTP_PROXY", "http_proxy"]).as_deref(),
        value(&["ALL_PROXY", "all_proxy"]).as_deref(),
        value(&["NO_PROXY", "no_proxy"]).as_deref(),
    )
    .or_else(ureq::Proxy::try_from_env)
}

fn proxy_from_values(
    https: Option<&str>,
    http: Option<&str>,
    all: Option<&str>,
    no_proxy: Option<&str>,
) -> Option<ureq::Proxy> {
    let proxy = [https, http, all]
        .into_iter()
        .flatten()
        .find_map(|value| ureq::Proxy::new(value).ok())?;
    let protocol = match proxy.protocol() {
        ureq::ProxyProtocol::Socks4 => ureq::ProxyProtocol::Socks4A,
        ureq::ProxyProtocol::Socks5 => ureq::ProxyProtocol::Socks5h,
        protocol => protocol,
    };
    let mut builder = ureq::Proxy::builder(protocol)
        .host(proxy.host())
        .port(proxy.port())
        .resolve_target(false);
    if let Some(username) = proxy.username() {
        builder = builder.username(username);
    }
    if let Some(password) = proxy.password() {
        builder = builder.password(password);
    }
    for expression in no_proxy.into_iter().flat_map(|value| value.split(',')) {
        builder = builder.no_proxy(expression.trim());
    }
    builder.build().ok().or(Some(proxy))
}

fn download_candidates(url: &str) -> Vec<String> {
    let mut candidates = vec![url.to_string()];
    if let Some(rest) = url.strip_prefix("https://raw.githubusercontent.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 4 {
            let (reference, path_start) = if parts.get(2..4) == Some(&["refs", "heads"]) {
                (parts.get(4).copied(), 5)
            } else {
                (parts.get(2).copied(), 3)
            };
            if let Some(reference) = reference.filter(|_| parts.len() > path_start) {
                candidates.push(format!(
                    "https://cdn.jsdelivr.net/gh/{}/{}@{}/{}",
                    parts[0],
                    parts[1],
                    reference,
                    parts[path_start..].join("/")
                ));
            }
        }
    } else if let Some(rest) = url.strip_prefix("https://cdn.jsdelivr.net/gh/")
        && let Some((owner, rest)) = rest.split_once('/')
        && let Some((repository_ref, path)) = rest.split_once('/')
        && let Some((repository, reference)) = repository_ref.rsplit_once('@')
    {
        candidates.push(format!(
            "https://raw.githubusercontent.com/{owner}/{repository}/{reference}/{path}"
        ));
    }
    candidates
}

fn is_github_url(url: &str) -> bool {
    [
        "github.com/",
        "raw.githubusercontent.com/",
        "githubusercontent.com/",
        "github.io/",
        "cdn.jsdelivr.net/gh/",
    ]
    .iter()
    .any(|host| url.contains(host))
}

fn bounded_concurrency(configured: usize, jobs: usize) -> usize {
    configured
        .clamp(MIN_DOWNLOAD_CONCURRENCY, MAX_DOWNLOAD_CONCURRENCY)
        .min(jobs.max(1))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadError(String);

impl fmt::Display for DownloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for DownloadError {}

#[cfg(test)]
mod tests {
    use super::{
        DownloadConfig, Downloader, bounded_concurrency, download_candidates, proxy_from_values,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    fn serve_once(status: &str, body: &str) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let body = body.to_string();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        (format!("http://{address}/rules.list"), server)
    }

    fn serve_socks5_once(body: &str) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let body = body.to_string();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut greeting = [0_u8; 2];
            stream.read_exact(&mut greeting).unwrap();
            let mut methods = vec![0_u8; greeting[1] as usize];
            stream.read_exact(&mut methods).unwrap();
            stream.write_all(&[5, 0]).unwrap();

            let mut request = [0_u8; 4];
            stream.read_exact(&mut request).unwrap();
            let address_len = match request[3] {
                1 => 4,
                3 => {
                    let mut length = [0_u8; 1];
                    stream.read_exact(&mut length).unwrap();
                    length[0] as usize
                }
                4 => 16,
                other => panic!("unexpected SOCKS address type {other}"),
            };
            let mut target = vec![0_u8; address_len + 2];
            stream.read_exact(&mut target).unwrap();
            stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).unwrap();

            let mut http_request = [0_u8; 1024];
            let _ = stream.read(&mut http_request);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        (format!("socks5://{address}"), server)
    }

    fn test_config(max_bytes: u64) -> DownloadConfig {
        DownloadConfig {
            attempts: 1,
            timeout: Duration::from_secs(2),
            batch_timeout: Duration::from_secs(5),
            backoff: Duration::ZERO,
            concurrency: 4,
            max_bytes,
            max_total_bytes: 1024,
        }
    }

    #[test]
    fn downloader_reads_bounded_http_source() {
        let (url, server) = serve_once("200 OK", "DOMAIN,ads.example.com");
        let result = Downloader::new(test_config(1024)).get(&url).unwrap();
        server.join().unwrap();

        assert_eq!(result, "DOMAIN,ads.example.com");
    }

    #[test]
    fn socks_proxy_feature_routes_requests() {
        let (proxy, server) = serve_socks5_once("proxied");
        let config = ureq::Agent::config_builder()
            .proxy(Some(ureq::Proxy::new(&proxy).unwrap()))
            .timeout_global(Some(Duration::from_secs(2)))
            .build();
        let mut response = ureq::Agent::new_with_config(config)
            .get("http://127.0.0.1:9/rules.list")
            .call()
            .unwrap();
        let body = response.body_mut().read_to_string().unwrap();
        server.join().unwrap();

        assert_eq!(body, "proxied");
    }

    #[test]
    fn https_proxy_precedes_socks_fallback_and_preserves_no_proxy() {
        let proxy = proxy_from_values(
            Some("http://127.0.0.1:6152"),
            None,
            Some("socks5://127.0.0.1:6153"),
            Some("localhost,127.0.0.1"),
        )
        .unwrap();
        let localhost = "http://localhost/rules.list".parse().unwrap();

        assert_eq!(proxy.protocol(), ureq::ProxyProtocol::Http);
        assert_eq!(proxy.port(), 6152);
        assert!(proxy.is_no_proxy(&localhost));

        let socks = proxy_from_values(None, None, Some("socks5://127.0.0.1:6153"), None).unwrap();
        assert_eq!(socks.protocol(), ureq::ProxyProtocol::Socks5h);
        assert!(!socks.resolve_target());
    }

    #[test]
    fn downloader_rejects_oversized_source() {
        let (url, server) = serve_once("200 OK", &"x".repeat(65));
        let result = Downloader::new(test_config(64)).get(&url);
        server.join().unwrap();

        assert!(result.is_err());
    }

    #[test]
    fn downloader_deduplicates_urls_and_collects_results() {
        let (url, server) = serve_once("200 OK", "DOMAIN,ads.example.com");
        let batch = Downloader::new(test_config(1024)).download_many([url.clone(), url.clone()]);
        server.join().unwrap();

        assert!(batch.failures.is_empty());
        assert_eq!(batch.contents.len(), 1);
        assert_eq!(batch.contents.get(&url).unwrap(), "DOMAIN,ads.example.com");
    }

    #[test]
    fn downloader_enforces_total_budget_before_retaining_all_bodies() {
        let (first_url, first_server) = serve_once("200 OK", "0123456789");
        let (second_url, second_server) = serve_once("200 OK", "abcdefghij");
        let mut config = test_config(64);
        config.max_total_bytes = 15;
        let batch = Downloader::new(config).download_many([first_url, second_url]);
        first_server.join().unwrap();
        second_server.join().unwrap();

        assert_eq!(batch.contents.len(), 1);
        assert_eq!(batch.failures.len(), 1);
    }

    #[test]
    fn github_raw_and_jsdelivr_are_bounded_mirror_candidates() {
        assert_eq!(
            download_candidates("https://raw.githubusercontent.com/acme/rules/main/ad.list"),
            [
                "https://raw.githubusercontent.com/acme/rules/main/ad.list",
                "https://cdn.jsdelivr.net/gh/acme/rules@main/ad.list",
            ]
        );
        assert_eq!(
            download_candidates("https://github.com/acme/rules/releases/latest/download/ad.list"),
            ["https://github.com/acme/rules/releases/latest/download/ad.list"]
        );
    }

    #[test]
    fn batch_concurrency_has_a_useful_floor_and_hard_ceiling() {
        assert_eq!(bounded_concurrency(0, 20), 1);
        assert_eq!(bounded_concurrency(1, 20), 1);
        assert_eq!(bounded_concurrency(4, 20), 4);
        assert_eq!(bounded_concurrency(99, 20), 6);
        assert_eq!(bounded_concurrency(6, 1), 1);
    }
}
