use std::collections::{BTreeSet, HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USER_AGENT: &str = "ScriptHub-PROMAX/1.0";
const MIN_DOWNLOAD_CONCURRENCY: usize = 2;
const MAX_DOWNLOAD_CONCURRENCY: usize = 6;
const GITHUB_REQUEST_INTERVAL: Duration = Duration::from_millis(125);
const MAX_RATE_LIMIT_DELAY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub attempts: usize,
    pub timeout: Duration,
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
    github_gate: Mutex<Option<Instant>>,
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
            .build();
        Self {
            agent: agent_config.into(),
            config,
            github_gate: Mutex::new(None),
        }
    }

    pub fn get(&self, url: &str) -> Result<String, DownloadError> {
        let attempts = self.config.attempts.max(1);
        let mut last_error = None;
        for attempt in 1..=attempts {
            self.wait_for_github_slot(url);
            let mut delay_override = None;
            let mut retryable = true;
            match self.agent.get(url).call() {
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
                        delay_override = Some(
                            response
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
                                    let now = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .ok()?
                                        .as_secs();
                                    Some(Duration::from_secs(reset.saturating_sub(now)))
                                })
                                .unwrap_or(MAX_RATE_LIMIT_DELAY)
                                .min(MAX_RATE_LIMIT_DELAY),
                        );
                    }
                    last_error = Some(format!("http status {status}"));
                }
                Err(error) => last_error = Some(error.to_string()),
            }

            if !retryable {
                break;
            }
            if attempt < attempts {
                let delay = delay_override
                    .unwrap_or_else(|| self.config.backoff.saturating_mul(attempt as u32));
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
            .unwrap_or(self.config.concurrency);
        requested_concurrency = bounded_concurrency(requested_concurrency, urls.len());

        let queue = Mutex::new(VecDeque::from_iter(urls));
        let results = Mutex::new(Vec::new());
        thread::scope(|scope| {
            for _ in 0..requested_concurrency {
                scope.spawn(|| {
                    loop {
                        let Some(url) = queue.lock().expect("download queue poisoned").pop_front()
                        else {
                            break;
                        };
                        let result = self.get(&url);
                        results
                            .lock()
                            .expect("download results poisoned")
                            .push((url, result));
                    }
                });
            }
        });

        let mut results = results.into_inner().expect("download results poisoned");
        results.sort_by(|left, right| left.0.cmp(&right.0));
        let mut batch = DownloadBatch::default();
        let mut total_bytes = 0_u64;
        for (url, result) in results {
            match result {
                Ok(content) => {
                    let next_total = total_bytes.saturating_add(content.len() as u64);
                    if next_total > self.config.max_total_bytes {
                        batch.failures.push(DownloadFailure {
                            url,
                            error: format!(
                                "PROMAX download batch exceeds {} bytes",
                                self.config.max_total_bytes
                            ),
                        });
                    } else {
                        total_bytes = next_total;
                        batch.contents.insert(url, content);
                    }
                }
                Err(error) => batch.failures.push(DownloadFailure {
                    url,
                    error: error.to_string(),
                }),
            }
        }
        batch
    }

    fn wait_for_github_slot(&self, url: &str) {
        if !is_github_url(url) {
            return;
        }
        let mut previous = self.github_gate.lock().expect("GitHub rate gate poisoned");
        if let Some(previous) = *previous {
            let remaining = GITHUB_REQUEST_INTERVAL.saturating_sub(previous.elapsed());
            if !remaining.is_zero() {
                thread::sleep(remaining);
            }
        }
        *previous = Some(Instant::now());
    }
}

fn is_github_url(url: &str) -> bool {
    [
        "github.com/",
        "raw.githubusercontent.com/",
        "githubusercontent.com/",
        "github.io/",
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
    use super::{DownloadConfig, Downloader, bounded_concurrency};
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

    fn test_config(max_bytes: u64) -> DownloadConfig {
        DownloadConfig {
            attempts: 1,
            timeout: Duration::from_secs(2),
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
    fn batch_concurrency_has_a_useful_floor_and_hard_ceiling() {
        assert_eq!(bounded_concurrency(0, 20), 2);
        assert_eq!(bounded_concurrency(4, 20), 4);
        assert_eq!(bounded_concurrency(99, 20), 6);
        assert_eq!(bounded_concurrency(6, 1), 1);
    }
}
