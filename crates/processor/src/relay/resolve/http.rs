//! HTTPS-only, fixed-provider DoH transport; no subscribed hostname uses system DNS.

use super::{ResolveError, Response, Transport, canonical, dns, wire};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use std::{
    io::Read,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};
use ureq::{
    http::{HeaderMap, Uri},
    unversioned::{
        resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver},
        transport::{DefaultConnector, NextTimeout},
    },
};

const BODY_LIMIT: u64 = 65_536;
const CLOUDFLARE: [IpAddr; 4] = [
    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
    IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1001)),
];
const GOOGLE: [IpAddr; 4] = [
    IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
    IpAddr::V4(Ipv4Addr::new(8, 8, 4, 4)),
    IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)),
    IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8844)),
];

#[derive(Debug)]
struct FixedResolver;
impl Resolver for FixedResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let ips = match uri.host().unwrap_or("") {
            "cloudflare-dns.com" => &CLOUDFLARE,
            "dns.google" => &GOOGLE,
            // ureq also resolves the explicitly configured proxy with this resolver.
            // DoH origins are fixed below and redirects are disabled.
            _ => return DefaultResolver::default().resolve(uri, config, timeout),
        };
        if uri.port_u16().unwrap_or(443) != 443 {
            return Err(ureq::Error::HostNotFound);
        }
        let mut out = self.empty();
        for ip in ips {
            out.push(SocketAddr::new(*ip, 443));
        }
        Ok(out)
    }
}

struct Request {
    url: String,
    accept: &'static str,
    body: Option<Vec<u8>>,
}
fn request(name: &str, kind: u16, options: &dns::Options) -> Result<Request, ResolveError> {
    if canonical(name).as_deref() != Some(name) || !matches!(kind, 1 | 28) {
        return Err(ResolveError::InvalidOptions);
    }
    let origin = match options.provider {
        dns::Provider::Cloudflare => "https://cloudflare-dns.com/dns-query",
        dns::Provider::Google => "https://dns.google/dns-query",
    };
    if options.format == dns::Format::Json {
        let (endpoint, extra) = if options.provider == dns::Provider::Google {
            (
                "https://dns.google/resolve",
                "&edns_client_subnet=0.0.0.0/0",
            )
        } else {
            (origin, "")
        };
        return Ok(Request {
            url: format!("{endpoint}?name={name}&type={kind}&cd=0{extra}"),
            accept: "application/dns-json",
            body: None,
        });
    }
    let packet = wire::query(name, kind)?;
    Ok(match options.format {
        dns::Format::Post => Request {
            url: origin.into(),
            accept: "application/dns-message",
            body: Some(packet),
        },
        _ => Request {
            url: format!("{origin}?dns={}", URL_SAFE_NO_PAD.encode(packet)),
            accept: "application/dns-message",
            body: None,
        },
    })
}

fn content_type(headers: &HeaderMap, format: dns::Format) -> bool {
    let mut values = headers.get_all("content-type").iter();
    let Some(value) = values.next().and_then(|v| v.to_str().ok()) else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    let mime = value.split(';').next().unwrap_or("").trim();
    if format == dns::Format::Json {
        mime.eq_ignore_ascii_case("application/dns-json")
            || mime.eq_ignore_ascii_case("application/json")
    } else {
        mime.eq_ignore_ascii_case("application/dns-message")
    }
}

fn retry_after(headers: &HeaderMap) -> Option<String> {
    // Presence itself requests a cooldown, even when the value cannot be decoded.
    headers
        .get_all("retry-after")
        .iter()
        .map(|value| super::retry_after(value.to_str().ok()))
        .max()
        .map(|seconds| seconds.to_string())
}

/// Revalidation is not implemented: ambiguous or non-cacheable replies are used once.
fn freshness(headers: &HeaderMap) -> (u64, Option<u64>, bool) {
    let ages: Vec<_> = headers.get_all("age").iter().collect();
    let parsed_age = ages
        .first()
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    let mut no_cache = ages.len() > 1 || (!ages.is_empty() && parsed_age.is_none());
    let mut max_age = None;
    for value in headers.get_all("cache-control") {
        let Ok(value) = value.to_str() else {
            no_cache = true;
            continue;
        };
        for directive in value.split(',') {
            let (key, value) = directive
                .trim()
                .split_once('=')
                .map_or((directive.trim(), None), |(k, v)| {
                    (k.trim(), Some(v.trim()))
                });
            if key.eq_ignore_ascii_case("no-cache") || key.eq_ignore_ascii_case("no-store") {
                no_cache = true;
            }
            if key.eq_ignore_ascii_case("max-age") {
                let parsed = value
                    .and_then(|v| {
                        v.strip_prefix('"')
                            .and_then(|v| v.strip_suffix('"'))
                            .or(Some(v))
                    })
                    .and_then(|v| v.parse::<u64>().ok());
                if parsed.is_none() || max_age.is_some() {
                    no_cache = true;
                }
                max_age = parsed;
            }
        }
    }
    (parsed_age.unwrap_or(0), max_age, no_cache)
}

pub(super) struct HttpTransport(ureq::Agent);
impl HttpTransport {
    pub(super) fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(8)))
            .build();
        Self(ureq::Agent::with_parts(
            config,
            DefaultConnector::default(),
            FixedResolver,
        ))
    }
}
impl Transport for HttpTransport {
    fn get(
        &self,
        name: &str,
        kind: u16,
        options: &dns::Options,
        timeout: Duration,
    ) -> Result<Response, ResolveError> {
        let request = request(name, kind, options)?;
        let mut response = if let Some(body) = request.body {
            self.0
                .post(&request.url)
                .config()
                .timeout_global(Some(timeout))
                .build()
                .header("accept", request.accept)
                .header("content-type", request.accept)
                .send(body.as_slice())
        } else {
            self.0
                .get(&request.url)
                .config()
                .timeout_global(Some(timeout))
                .build()
                .header("accept", request.accept)
                .call()
        }
        .map_err(|_| ResolveError::Unavailable)?;
        let status = response.status().as_u16();
        let retry_after = retry_after(response.headers());
        let (age, max_age, no_cache) = freshness(response.headers());
        let mut body = Vec::new();
        if status == 200 {
            if !content_type(response.headers(), options.format) {
                return Err(ResolveError::InvalidResponse);
            }
            response
                .body_mut()
                .as_reader()
                .take(BODY_LIMIT + 1)
                .read_to_end(&mut body)
                .map_err(|_| ResolveError::Unavailable)?;
            if body.len() as u64 > BODY_LIMIT {
                return Err(ResolveError::InvalidResponse);
            }
        }
        Ok(Response {
            status,
            retry_after,
            body,
            age,
            no_cache,
            max_age,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requests_keep_canonical_hosts_in_fixed_https_origins() {
        for provider in [dns::Provider::Cloudflare, dns::Provider::Google] {
            for kind in [1, 28] {
                let mut options = dns::Options {
                    provider,
                    ..Default::default()
                };
                let post = request("example.com", kind, &options).unwrap();
                assert!(!post.url.contains('?'));
                let bytes = post.body.unwrap();
                let message = hickory_proto::op::Message::from_vec(&bytes).unwrap();
                assert_eq!(
                    message.queries[0].query_type,
                    hickory_proto::rr::RecordType::from(kind)
                );
                options.format = dns::Format::Get;
                let get = request("example.com", kind, &options).unwrap();
                assert!(get.body.is_none());
                let encoded = get.url.split_once("?dns=").unwrap().1;
                assert!(!encoded.contains('='));
                assert_eq!(URL_SAFE_NO_PAD.decode(encoded).unwrap(), bytes);
                options.format = dns::Format::Json;
                let json = request("example.com", kind, &options).unwrap();
                assert!(json.url.contains("cd=0"));
                assert_eq!(
                    json.url.contains("/resolve?"),
                    provider == dns::Provider::Google
                );
                assert_eq!(
                    json.url.contains("edns_client_subnet=0.0.0.0/0"),
                    provider == dns::Provider::Google
                );
                assert!(request("bad&name=secret", kind, &options).is_err());
            }
        }
    }
    #[test]
    fn response_type_and_cache_headers_are_validated() {
        let mut headers = HeaderMap::new();
        assert!(!content_type(&headers, dns::Format::Post));
        headers.insert("content-type", "application/dns-message".parse().unwrap());
        assert!(content_type(&headers, dns::Format::Get));
        assert!(!content_type(&headers, dns::Format::Json));
        headers.insert(
            "content-type",
            "application/json; charset=UTF-8".parse().unwrap(),
        );
        assert!(content_type(&headers, dns::Format::Json));
        assert_eq!(freshness(&headers), (0, None, false));
        headers.insert("age", "4".parse().unwrap());
        headers.insert("cache-control", "public, max-age=30".parse().unwrap());
        assert_eq!(freshness(&headers), (4, Some(30), false));
        headers.insert("cache-control", "max-age=0".parse().unwrap());
        assert_eq!(freshness(&headers), (4, Some(0), false));
        for value in [
            "no-store",
            "no-cache=\"field\"",
            "max-age=abc",
            "max-age=30, max-age=60",
        ] {
            headers.insert("cache-control", value.parse().unwrap());
            assert!(freshness(&headers).2);
        }
        headers.insert("cache-control", "max-age=\"30\"".parse().unwrap());
        assert_eq!(freshness(&headers), (4, Some(30), false));
        headers.append("age", "5".parse().unwrap());
        assert!(freshness(&headers).2);
        headers.remove("age");
        headers.insert("age", "invalid".parse().unwrap());
        assert!(freshness(&headers).2);
        headers.insert("retry-after", "120".parse().unwrap());
        headers.append("retry-after", "7200".parse().unwrap());
        assert_eq!(retry_after(&headers), Some("7200".into()));
        headers.insert("retry-after", "invalid".parse().unwrap());
        assert_eq!(retry_after(&headers), Some("60".into()));
    }
}
