use std::{io::Read, sync::atomic::AtomicBool, time::Duration};

use ureq::unversioned::{
    resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver},
    transport::{DefaultConnector, NextTimeout},
};
use url::{Host, Url};
use zeroize::Zeroizing;

use super::{RelayError, active, gist, model, resolve};

const LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Clone)]
pub enum Source {
    Gist(gist::Source),
    Https {
        url: Zeroizing<String>,
        content: Zeroizing<String>,
    },
}

impl Source {
    pub fn content(&self) -> &str {
        match self {
            Self::Gist(source) => &source.content,
            Self::Https { content, .. } => content,
        }
    }
    pub fn gist_file(&self) -> Option<&gist::GistFile> {
        match self {
            Self::Gist(source) => Some(&source.file),
            Self::Https { .. } => None,
        }
    }
    fn same_identity(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Gist(a), Self::Gist(b)) => {
                a.file.id == b.file.id && a.file.filename == b.file.filename
            }
            (Self::Https { url: a, .. }, Self::Https { url: b, .. }) => a == b,
            _ => false,
        }
    }
    pub(super) fn matches(&self, other: &Self) -> bool {
        self.same_identity(other) && model::semantic_equal(self.content(), other.content())
    }
}

pub(super) enum Target {
    Gist(gist::GistFile),
    Https(Url),
}

pub(super) fn target(input: &str, filename: &str) -> Result<Target, RelayError> {
    let gist_error = match gist::GistFile::parse_source(input, filename) {
        Ok(file) => return Ok(Target::Gist(file)),
        Err(error) => error,
    };
    let url = match Url::parse(input) {
        Ok(url) => url,
        Err(_) => return Err(RelayError::Gist(gist_error)),
    };
    if [
        "gist.github.com",
        "api.github.com",
        "gist.githubusercontent.com",
    ]
    .contains(&url.host_str().unwrap_or(""))
    {
        return Err(RelayError::Gist(gist_error));
    }
    if url.scheme() != "https"
        || input.split_once("://").is_some_and(|(_, rest)| {
            rest.split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .contains('@')
        })
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !filename.is_empty()
    {
        return Err(RelayError::InvalidInput);
    }
    match url.host() {
        Some(Host::Domain(host))
            if host.contains('.')
                && !host.ends_with('.')
                && !["localhost", "local", "internal", "lan", "home.arpa"]
                    .iter()
                    .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}"))) => {}
        Some(Host::Ipv4(ip)) if resolve::public(ip.into()) => {}
        Some(Host::Ipv6(ip)) if resolve::public(ip.into()) => {}
        _ => return Err(RelayError::InvalidInput),
    }
    Ok(Target::Https(url))
}

#[derive(Debug)]
struct PublicResolver;
impl Resolver for PublicResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let addresses = DefaultResolver::default().resolve(uri, config, timeout)?;
        if !public_addresses(&addresses) {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(addresses)
    }
}

fn public_addresses(addresses: &ResolvedSocketAddrs) -> bool {
    !addresses.is_empty()
        && addresses
            .iter()
            .all(|address| resolve::public(address.ip()))
}

fn status_error(
    status: u16,
    headers: &ureq::http::HeaderMap,
    now: chrono::DateTime<chrono::Utc>,
) -> RelayError {
    let retry = headers.get_all("retry-after");
    if status == 429
        || ((status == 403 || (500..600).contains(&status)) && retry.iter().next().is_some())
    {
        let retry_after_seconds = retry
            .iter()
            .filter_map(|value| {
                let value = value.to_str().ok()?.trim();
                super::parse_retry_delay_seconds(value).or_else(|| {
                    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
                    let millis = date.signed_duration_since(now).num_milliseconds().max(0) as u64;
                    Some(millis.div_ceil(1000))
                })
            })
            .max();
        RelayError::InputRateLimited {
            retry_after_seconds,
        }
    } else {
        RelayError::InputHttpStatus(status)
    }
}

pub(super) fn read(
    client: &gist::GistClient,
    input: &str,
    filename: &str,
    cancel: &AtomicBool,
) -> Result<Source, RelayError> {
    match target(input, filename)? {
        Target::Gist(_) => client
            .read_source(input, filename, cancel)
            .map(Source::Gist)
            .map_err(RelayError::Gist),
        Target::Https(url) => {
            active(cancel)?;
            let config = ureq::Agent::config_builder()
                .https_only(true)
                .proxy(None)
                .max_redirects(0)
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(20)))
                .timeout_connect(Some(Duration::from_secs(5)))
                .build();
            let agent =
                ureq::Agent::with_parts(config, DefaultConnector::default(), PublicResolver);
            let mut response = agent
                .get(url.as_str())
                .call()
                .map_err(|_| RelayError::InputUnavailable)?;
            if response.status().as_u16() != 200 {
                return Err(status_error(
                    response.status().as_u16(),
                    response.headers(),
                    chrono::Utc::now(),
                ));
            }
            let mut bytes = Zeroizing::new(Vec::new());
            response
                .body_mut()
                .as_reader()
                .take(LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| RelayError::InputUnavailable)?;
            active(cancel)?;
            if bytes.len() as u64 > LIMIT {
                return Err(RelayError::InputInvalid);
            }
            let content = std::str::from_utf8(&bytes).map_err(|_| RelayError::InputInvalid)?;
            Ok(Source::Https {
                url: Zeroizing::new(url.to_string()),
                content: Zeroizing::new(content.to_owned()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_https_target_without_network() {
        assert!(matches!(
            target("https://example.com/sub?key=secret", ""),
            Ok(Target::Https(_))
        ));
        for url in [
            "http://example.com/sub",
            "https://user@example.com/",
            "https://@example.com/",
            "https://example.com/#fragment",
            "https://localhost/",
            "https://printer.local/",
            "https://127.0.0.1/",
            "https://10.0.0.1/",
            "https://169.254.169.254/",
            "https://[::1]/",
            "https://[::ffff:127.0.0.1]/",
            "https://192.0.2.1/",
        ] {
            assert!(target(url, "").is_err(), "unsafe target accepted");
        }
        assert!(target("https://example.com/", "file").is_err());
        assert!(matches!(
            target("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", ""),
            Ok(Target::Gist(_))
        ));
    }

    #[test]
    fn source_identity_and_content_must_match() {
        let source = Source::Https {
            url: Zeroizing::new("https://example.com/a".into()),
            content: Zeroizing::new("A = socks5, example.com, 1080\n".into()),
        };
        let other = Source::Https {
            url: Zeroizing::new("https://example.com/b".into()),
            content: Zeroizing::new(source.content().into()),
        };
        assert!(source.matches(&source));
        assert!(!source.matches(&other));
        assert!(model::Subscription::parse("<html><body>Unavailable</body></html>").is_err());
    }

    #[test]
    fn rejects_mixed_dns_answers() {
        let resolver = PublicResolver;
        let mut addresses = resolver.empty();
        assert!(!public_addresses(&addresses));
        addresses.push("1.1.1.1:443".parse().unwrap());
        assert!(public_addresses(&addresses));
        addresses.push("127.0.0.1:443".parse().unwrap());
        assert!(!public_addresses(&addresses));
    }

    #[test]
    fn status_and_retry_after_are_classified_without_exposing_headers() {
        let now = chrono::DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT")
            .unwrap()
            .to_utc();
        let mut headers = ureq::http::HeaderMap::new();
        assert!(matches!(
            status_error(401, &headers, now),
            RelayError::InputHttpStatus(401)
        ));
        assert!(matches!(
            status_error(403, &headers, now),
            RelayError::InputHttpStatus(403)
        ));
        assert!(matches!(
            status_error(302, &headers, now),
            RelayError::InputHttpStatus(302)
        ));
        assert!(matches!(
            status_error(429, &headers, now),
            RelayError::InputRateLimited {
                retry_after_seconds: None
            }
        ));
        headers.append("retry-after", "60".parse().unwrap());
        headers.append(
            "retry-after",
            "Wed, 21 Oct 2015 07:30:00 GMT".parse().unwrap(),
        );
        for status in [403, 429, 503] {
            assert!(matches!(
                status_error(status, &headers, now),
                RelayError::InputRateLimited {
                    retry_after_seconds: Some(120)
                }
            ));
        }
        assert!(matches!(
            status_error(302, &headers, now),
            RelayError::InputHttpStatus(302)
        ));
        headers.append("retry-after", "18446744073709551616".parse().unwrap());
        assert!(matches!(
            status_error(503, &headers, now),
            RelayError::InputRateLimited {
                retry_after_seconds: Some(u64::MAX)
            }
        ));
        headers.insert("retry-after", "secret-invalid".parse().unwrap());
        let error = status_error(429, &headers, now);
        assert!(matches!(
            error,
            RelayError::InputRateLimited {
                retry_after_seconds: None
            }
        ));
        assert!(!error.to_string().contains("secret-invalid"));
    }
}
