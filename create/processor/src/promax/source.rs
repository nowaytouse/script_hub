use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fmt;
use std::thread;
use std::time::Duration;

const USER_AGENT: &str = "ScriptHub-PROMAX/1.0";

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub attempts: usize,
    pub timeout: Duration,
    pub backoff: Duration,
    pub max_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            attempts: 3,
            timeout: Duration::from_secs(60),
            backoff: Duration::from_secs(1),
            max_bytes: 64 * 1024 * 1024,
            max_total_bytes: 256 * 1024 * 1024,
        }
    }
}

pub struct Downloader {
    agent: ureq::Agent,
    config: DownloadConfig,
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
            .user_agent(USER_AGENT)
            .build();
        Self {
            agent: agent_config.into(),
            config,
        }
    }

    pub fn get(&self, url: &str) -> Result<String, DownloadError> {
        let attempts = self.config.attempts.max(1);
        let mut last_error = None;
        for attempt in 1..=attempts {
            let result = self.agent.get(url).call().and_then(|mut response| {
                response
                    .body_mut()
                    .with_config()
                    .limit(self.config.max_bytes)
                    .read_to_string()
            });
            match result {
                Ok(body) => return Ok(body),
                Err(error) => last_error = Some(error.to_string()),
            }

            if attempt < attempts && !self.config.backoff.is_zero() {
                thread::sleep(self.config.backoff.saturating_mul(attempt as u32));
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
        let mut batch = DownloadBatch::default();
        let mut total_bytes = 0_u64;
        for url in urls {
            match self.get(&url) {
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
    use super::{DownloadConfig, Downloader};
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
}
