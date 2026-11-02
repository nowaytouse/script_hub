//! Explicitly invoked, bounded DNS-over-HTTPS resolution.
use super::model::pipeline::{AddressFamily, ResolveError};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use ureq::unversioned::{
    resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver},
    transport::{DefaultConnector, NextTimeout},
};
use zeroize::Zeroize;

const LIMIT: usize = 1024;
const BODY_LIMIT: u64 = 65_536;
const BUDGET: Duration = Duration::from_secs(120);

#[derive(Debug)]
struct FixedResolver;
impl Resolver for FixedResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        if uri.host() != Some("cloudflare-dns.com") || uri.port_u16().unwrap_or(443) != 443 {
            // Only a configured proxy can introduce another connection target.
            return DefaultResolver::default().resolve(uri, config, timeout);
        }
        let mut addresses = self.empty();
        addresses.push(SocketAddr::from((Ipv4Addr::new(1, 1, 1, 1), 443)));
        Ok(addresses)
    }
}

#[derive(Clone, Copy)]
struct Cached {
    value: Option<IpAddr>,
    expires: Instant,
}
struct State {
    cache: HashMap<(String, u16), Cached>,
    requests: usize,
}
impl Drop for State {
    fn drop(&mut self) {
        for ((mut name, _), _) in self.cache.drain() {
            name.zeroize();
        }
    }
}
static COOLDOWN: Mutex<Option<(Instant, u64)>> = Mutex::new(None);
static LIMITER: Mutex<(Option<Instant>, usize)> = Mutex::new((None, 0));
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        if let Ok(mut limiter) = LIMITER.lock() {
            limiter.1 = limiter.1.saturating_sub(1);
        }
    }
}

struct Response {
    status: u16,
    retry_after: Option<String>,
    body: Vec<u8>,
}
trait Transport: Send + Sync {
    fn get(&self, name: &str, kind: u16, timeout: Duration) -> Result<Response, ResolveError>;
}
struct HttpTransport(ureq::Agent);
impl Transport for HttpTransport {
    fn get(&self, name: &str, kind: u16, timeout: Duration) -> Result<Response, ResolveError> {
        let url = format!("https://cloudflare-dns.com/dns-query?name={name}&type={kind}");
        let mut response = self
            .0
            .get(&url)
            .config()
            .timeout_global(Some(timeout))
            .build()
            .header("accept", "application/dns-json")
            .call()
            .map_err(|_| ResolveError::Unavailable)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut body = Vec::new();
        if status == 200 {
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
        })
    }
}

/// A resolution session. Calling it represents the caller's explicit consent.
pub struct Session<'a> {
    cancel: &'a AtomicBool,
    transport: Arc<dyn Transport>,
    state: Mutex<State>,
    batch: Mutex<()>,
    deadline: Instant,
}
impl<'a> Session<'a> {
    pub fn new(cancel: &'a AtomicBool) -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(8)))
            .build();
        let agent = ureq::Agent::with_parts(config, DefaultConnector::default(), FixedResolver);
        Self::with_transport(cancel, Arc::new(HttpTransport(agent)))
    }
    fn with_transport(cancel: &'a AtomicBool, transport: Arc<dyn Transport>) -> Self {
        Self {
            cancel,
            transport,
            state: Mutex::new(State {
                cache: HashMap::new(),
                requests: 0,
            }),
            batch: Mutex::new(()),
            deadline: Instant::now() + BUDGET,
        }
    }
    pub fn resolve(
        &self,
        hosts: &[&str],
        family: AddressFamily,
    ) -> Result<Vec<Option<IpAddr>>, ResolveError> {
        let deadline = self.deadline;
        let _batch = loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(ResolveError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(ResolveError::Budget);
            }
            match self.batch.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::WouldBlock) => {
                    thread::sleep(Duration::from_millis(25))
                }
                Err(std::sync::TryLockError::Poisoned(_)) => return Err(ResolveError::Unavailable),
            }
        };
        let mut unique = Vec::new();
        let mut indices = HashMap::new();
        let mut mapping = Vec::with_capacity(hosts.len());
        let mut literals = Vec::with_capacity(hosts.len());
        for host in hosts {
            if let Ok(ip) = host.parse::<IpAddr>() {
                literals.push(Some(match family {
                    AddressFamily::V4 if !ip.is_ipv4() => None,
                    AddressFamily::V6 if !ip.is_ipv6() => None,
                    _ => Some(ip),
                }));
                mapping.push(0);
            } else if let Some(name) = canonical(host) {
                literals.push(None);
                let index = *indices.entry(name.clone()).or_insert_with(|| {
                    let i = unique.len();
                    unique.push(name);
                    i
                });
                mapping.push(index);
            } else {
                literals.push(Some(None));
                mapping.push(0);
            }
        }
        if unique.len() > LIMIT {
            return Err(ResolveError::Budget);
        }
        let results = Mutex::new(vec![None; unique.len()]);
        let next = AtomicUsize::new(0);
        let failure = Mutex::new(None);
        thread::scope(|scope| {
            for _ in 0..unique.len().min(4) {
                scope.spawn(|| {
                    loop {
                        if self.cancel.load(Ordering::SeqCst)
                            || failure.lock().map_or(true, |e| e.is_some())
                        {
                            break;
                        }
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(name) = unique.get(index) else {
                            break;
                        };
                        let types: &[u16] = match family {
                            AddressFamily::V4 => &[1],
                            AddressFamily::V6 => &[28],
                            AddressFamily::PreferV4 => &[1, 28],
                            AddressFamily::PreferV6 => &[28, 1],
                        };
                        let mut value = None;
                        for &kind in types {
                            match self.query(name, kind, deadline, &failure) {
                                Ok(Some(ip)) => {
                                    value = Some(ip);
                                    break;
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    if let Ok(mut f) = failure.lock()
                                        && f.is_none()
                                    {
                                        *f = Some(e);
                                    }
                                    break;
                                }
                            }
                        }
                        if let Ok(mut out) = results.lock() {
                            out[index] = Some(value);
                        }
                    }
                });
            }
        });
        if self.cancel.load(Ordering::SeqCst) {
            return Err(ResolveError::Cancelled);
        }
        if let Some(error) = *failure.lock().map_err(|_| ResolveError::Unavailable)? {
            return Err(error);
        }
        let results = results
            .into_inner()
            .map_err(|_| ResolveError::Unavailable)?;
        mapping
            .into_iter()
            .zip(literals)
            .map(|(i, literal)| {
                literal.map(Ok).unwrap_or_else(|| {
                    results
                        .get(i)
                        .copied()
                        .flatten()
                        .ok_or(ResolveError::Unavailable)
                })
            })
            .collect()
    }
    fn query(
        &self,
        name: &str,
        kind: u16,
        deadline: Instant,
        failure: &Mutex<Option<ResolveError>>,
    ) -> Result<Option<IpAddr>, ResolveError> {
        let _permit = loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(ResolveError::Cancelled);
            }
            if failure
                .lock()
                .map_err(|_| ResolveError::Unavailable)?
                .is_some()
            {
                return Err(ResolveError::Unavailable);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(ResolveError::Budget);
            }
            let mut state = self.state.lock().map_err(|_| ResolveError::Unavailable)?;
            if let Some(entry) = state
                .cache
                .get(&(name.to_owned(), kind))
                .filter(|e| e.expires > now)
            {
                return Ok(entry.value);
            }
            if let Some((start, seconds)) =
                *COOLDOWN.lock().map_err(|_| ResolveError::Unavailable)?
            {
                let remaining =
                    seconds.saturating_sub(now.saturating_duration_since(start).as_secs());
                if remaining > 0 {
                    return Err(ResolveError::RateLimited {
                        retry_after_seconds: remaining,
                    });
                }
            }
            if state.requests >= LIMIT {
                return Err(ResolveError::Budget);
            }
            let mut limiter = LIMITER.lock().map_err(|_| ResolveError::Unavailable)?;
            if limiter.1 >= 4 || limiter.0.is_some_and(|next| now < next) {
                drop(limiter);
                drop(state);
                thread::sleep(Duration::from_millis(25));
                continue;
            }
            limiter.0 = Some(now + Duration::from_millis(125));
            limiter.1 += 1;
            state.requests += 1;
            break Permit;
        };
        if self.cancel.load(Ordering::SeqCst) {
            return Err(ResolveError::Cancelled);
        }
        if failure
            .lock()
            .map_err(|_| ResolveError::Unavailable)?
            .is_some()
        {
            return Err(ResolveError::Unavailable);
        }
        if let Some((start, seconds)) = *COOLDOWN.lock().map_err(|_| ResolveError::Unavailable)? {
            let remaining =
                seconds.saturating_sub(Instant::now().saturating_duration_since(start).as_secs());
            if remaining > 0 {
                return Err(ResolveError::RateLimited {
                    retry_after_seconds: remaining,
                });
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ResolveError::Budget);
        }
        let response = self
            .transport
            .get(name, kind, remaining.min(Duration::from_secs(8)))?;
        if response.status == 429 || (response.status != 200 && response.retry_after.is_some()) {
            let seconds = retry_after(response.retry_after.as_deref());
            let mut cooldown = COOLDOWN.lock().map_err(|_| ResolveError::Unavailable)?;
            let now = Instant::now();
            let old = cooldown
                .map(|(start, duration)| {
                    duration.saturating_sub(now.saturating_duration_since(start).as_secs())
                })
                .unwrap_or(0);
            *cooldown = Some((now, seconds.max(old)));
            return Err(ResolveError::RateLimited {
                retry_after_seconds: seconds,
            });
        }
        if response.status != 200 {
            return Err(ResolveError::Unavailable);
        }
        let (value, ttl) = parse_response(&response.body, name, kind)?;
        if ttl > 0 {
            self.state
                .lock()
                .map_err(|_| ResolveError::Unavailable)?
                .cache
                .insert(
                    (name.to_owned(), kind),
                    Cached {
                        value,
                        expires: Instant::now() + Duration::from_secs(ttl.min(86400)),
                    },
                );
        }
        Ok(value)
    }
}

fn retry_after(value: Option<&str>) -> u64 {
    let Some(value) = value else {
        return 60;
    };
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return seconds.max(1);
    }
    if let Ok(date) = chrono::DateTime::parse_from_rfc2822(value) {
        return date
            .signed_duration_since(chrono::Utc::now())
            .num_seconds()
            .saturating_add(1)
            .max(1) as u64;
    }
    60
}

fn canonical(name: &str) -> Option<String> {
    let name = name.strip_suffix('.').unwrap_or(name);
    if name.is_empty() || name.len() > 253 || !name.is_ascii() {
        return None;
    }
    let name = name.to_ascii_lowercase();
    if name == "localhost" || name.ends_with(".localhost") || name.ends_with(".local") {
        return None;
    }
    if name.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return None;
    }
    Some(name)
}

#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "Status")]
    status: u16,
    #[serde(rename = "TC")]
    truncated: bool,
    #[serde(rename = "Question")]
    question: Vec<Question>,
    #[serde(rename = "Answer", default)]
    answer: Vec<Record>,
}
#[derive(Deserialize)]
struct Question {
    name: String,
    #[serde(rename = "type")]
    kind: u16,
}
#[derive(Deserialize)]
struct Record {
    name: String,
    #[serde(rename = "type")]
    kind: u16,
    #[serde(rename = "TTL")]
    ttl: u64,
    data: String,
}
fn parse_response(
    bytes: &[u8],
    host: &str,
    kind: u16,
) -> Result<(Option<IpAddr>, u64), ResolveError> {
    let reply: Reply = serde_json::from_slice(bytes).map_err(|_| ResolveError::InvalidResponse)?;
    if reply.truncated
        || reply.question.len() != 1
        || canonical(&reply.question[0].name).as_deref() != Some(host)
        || reply.question[0].kind != kind
    {
        return Err(ResolveError::InvalidResponse);
    }
    if reply.status == 3 && reply.answer.is_empty() {
        return Ok((None, 0));
    }
    if reply.status != 0 {
        return Err(ResolveError::Unavailable);
    }
    let mut names = HashSet::new();
    let mut current = host.to_owned();
    let mut ttl = u64::MAX;
    loop {
        if !names.insert(current.clone()) {
            return Err(ResolveError::InvalidResponse);
        }
        let aliases: Vec<_> = reply
            .answer
            .iter()
            .filter(|r| r.kind == 5 && canonical(&r.name).as_deref() == Some(current.as_str()))
            .collect();
        if aliases.is_empty() {
            break;
        }
        if aliases.len() != 1 {
            return Err(ResolveError::InvalidResponse);
        }
        ttl = ttl.min(aliases[0].ttl);
        current = canonical(&aliases[0].data).ok_or(ResolveError::InvalidResponse)?;
    }
    let mut found = Vec::new();
    for record in &reply.answer {
        let owner = canonical(&record.name).ok_or(ResolveError::InvalidResponse)?;
        if record.kind == 5 {
            if !names.contains(&owner) {
                return Err(ResolveError::InvalidResponse);
            }
            continue;
        }
        if record.kind != kind || owner != current {
            return Err(ResolveError::InvalidResponse);
        }
        let ip: IpAddr = record
            .data
            .parse()
            .map_err(|_| ResolveError::InvalidResponse)?;
        if (kind == 1) != ip.is_ipv4() {
            return Err(ResolveError::InvalidResponse);
        }
        ttl = ttl.min(record.ttl);
        if public(ip) {
            found.push(ip);
        }
    }
    found.sort_unstable();
    Ok((
        found.first().copied(),
        if ttl == u64::MAX { 0 } else { ttl },
    ))
}
fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_multicast()
                || v.is_broadcast()
                || o[0] == 0
                || o[0] >= 240
                || (o[0] == 100 && (64..=127).contains(&o[1]))
                || (o[0] == 192
                    && o[1] == 0
                    && ((o[2] == 0 && o[3] != 9 && o[3] != 10) || o[2] == 2))
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19 || (o[1] == 51 && o[2] == 100)))
                || (o[0] == 203 && o[1] == 0 && o[2] == 113))
        }
        IpAddr::V6(v) => {
            !v.is_loopback()
                && !v.is_unspecified()
                && !v.is_multicast()
                && !v.is_unique_local()
                && !v.is_unicast_link_local()
                && v.to_ipv4_mapped().is_none()
                && !(v.segments()[0] == 0x2001 && v.segments()[1] == 0x0db8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct Fixture {
        calls: AtomicUsize,
        active: AtomicUsize,
        peak: AtomicUsize,
        mode: AtomicUsize,
        cancel: Arc<AtomicBool>,
    }
    impl Fixture {
        fn new(cancel: Arc<AtomicBool>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                active: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                mode: AtomicUsize::new(0),
                cancel,
            }
        }
    }
    impl Transport for Fixture {
        fn get(&self, name: &str, kind: u16, _: Duration) -> Result<Response, ResolveError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(active, Ordering::SeqCst);
            let mode = self.mode.load(Ordering::SeqCst);
            if mode == 0 {
                thread::sleep(Duration::from_millis(300));
            }
            if mode == 1 {
                self.cancel.store(true, Ordering::SeqCst);
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            if mode == 2 {
                return Ok(Response {
                    status: 429,
                    retry_after: Some("7200".into()),
                    body: vec![],
                });
            }
            if mode == 3 {
                return Ok(Response {
                    status: 503,
                    retry_after: Some("120".into()),
                    body: vec![],
                });
            }
            if mode == 4 {
                return Ok(Response {
                    status: 503,
                    retry_after: Some(
                        (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc2822(),
                    ),
                    body: vec![],
                });
            }
            let data = if kind == 1 {
                "1.1.1.1"
            } else {
                "2606:4700:4700::1111"
            };
            Ok(Response { status: 200, retry_after: None, body: serde_json::json!({"Status": 0, "TC": false, "Question": [{"name": name, "type": kind}], "Answer": [{"name": name, "type": kind, "TTL": 1, "data": data}]}).to_string().into_bytes() })
        }
    }

    #[test]
    fn injected_transport_covers_batch_cache_cancel_and_cooldown() {
        let cancel = Arc::new(AtomicBool::new(false));
        let fixture = Arc::new(Fixture::new(cancel.clone()));
        let session = Session::with_transport(&cancel, fixture.clone());
        let out = session
            .resolve(
                &[
                    "a.example",
                    "A.EXAMPLE.",
                    "b.example",
                    "c.example",
                    "d.example",
                ],
                AddressFamily::V4,
            )
            .unwrap();
        assert_eq!(out[0], out[1]);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 4);
        assert!((2..=4).contains(&fixture.peak.load(Ordering::SeqCst)));
        let before = fixture.calls.load(Ordering::SeqCst);
        session.resolve(&["d.example"], AddressFamily::V4).unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), before);
        thread::sleep(Duration::from_millis(1100));
        session.resolve(&["d.example"], AddressFamily::V4).unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), before + 1);
        fixture.mode.store(1, Ordering::SeqCst);
        assert_eq!(
            session.resolve(&["cancel.example"], AddressFamily::V4),
            Err(ResolveError::Cancelled)
        );
        cancel.store(false, Ordering::SeqCst);
        fixture.mode.store(2, Ordering::SeqCst);
        assert_eq!(
            session.resolve(&["limited.example"], AddressFamily::V4),
            Err(ResolveError::RateLimited {
                retry_after_seconds: 7200
            })
        );
        let another = Session::with_transport(&cancel, fixture.clone());
        let calls = fixture.calls.load(Ordering::SeqCst);
        assert!(matches!(
            another.resolve(&["other.example"], AddressFamily::V4),
            Err(ResolveError::RateLimited {
                retry_after_seconds: 7199..=7200
            })
        ));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), calls);
        *COOLDOWN.lock().unwrap() = None;
        fixture.mode.store(3, Ordering::SeqCst);
        assert_eq!(
            session.resolve(&["retry-numeric.example"], AddressFamily::V4),
            Err(ResolveError::RateLimited {
                retry_after_seconds: 120
            })
        );
        let calls = fixture.calls.load(Ordering::SeqCst);
        assert!(matches!(
            another.resolve(&["blocked.example"], AddressFamily::V4),
            Err(ResolveError::RateLimited {
                retry_after_seconds: 119..=120
            })
        ));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), calls);
        *COOLDOWN.lock().unwrap() = None;
        fixture.mode.store(4, Ordering::SeqCst);
        assert!(matches!(
            session.resolve(&["retry-date.example"], AddressFamily::V4),
            Err(ResolveError::RateLimited {
                retry_after_seconds: 119..=121
            })
        ));
        let calls = fixture.calls.load(Ordering::SeqCst);
        assert!(matches!(
            another.resolve(&["blocked-date.example"], AddressFamily::V4),
            Err(ResolveError::RateLimited {
                retry_after_seconds: 118..=121
            })
        ));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), calls);
        *COOLDOWN.lock().unwrap() = None;
    }

    #[test]
    fn parses_retry_after_without_shortening_server_minimum() {
        assert_eq!(retry_after(Some("7200")), 7200);
        assert_eq!(retry_after(Some("not-a-date")), 60);
        let future = (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc2822();
        assert!(retry_after(Some(&future)) >= 119);
    }

    #[test]
    fn expired_session_never_dispatches() {
        let cancel = Arc::new(AtomicBool::new(false));
        let fixture = Arc::new(Fixture::new(cancel.clone()));
        let mut session = Session::with_transport(&cancel, fixture.clone());
        session.deadline = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            session.resolve(&["expired.example"], AddressFamily::V4),
            Err(ResolveError::Budget)
        );
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn validates_question_and_selects_lowest_public_address() {
        let body = br#"{"Status":0,"TC":false,"Question":[{"name":"example.com.","type":1}],"Answer":[{"name":"example.com.","type":1,"TTL":30,"data":"8.8.8.8"},{"name":"example.com.","type":1,"TTL":20,"data":"1.1.1.1"},{"name":"example.com.","type":1,"TTL":10,"data":"10.0.0.1"}]}"#;
        assert_eq!(
            parse_response(body, "example.com", 1),
            Ok((Some("1.1.1.1".parse().unwrap()), 10))
        );
        assert_eq!(
            parse_response(body, "other.com", 1),
            Err(ResolveError::InvalidResponse)
        );
    }

    #[test]
    fn rejects_unrelated_answers_and_alias_loops() {
        let unrelated = br#"{"Status":0,"TC":false,"Question":[{"name":"example.com.","type":28}],"Answer":[{"name":"other.com.","type":28,"TTL":30,"data":"2606:4700:4700::1111"}]}"#;
        assert_eq!(
            parse_response(unrelated, "example.com", 28),
            Err(ResolveError::InvalidResponse)
        );
        let cycle = br#"{"Status":0,"TC":false,"Question":[{"name":"example.com.","type":1}],"Answer":[{"name":"example.com.","type":5,"TTL":30,"data":"example.com."}]}"#;
        assert_eq!(
            parse_response(cycle, "example.com", 1),
            Err(ResolveError::InvalidResponse)
        );
    }

    #[test]
    fn handles_negative_and_ipv6_answers() {
        let negative = br#"{"Status":3,"TC":false,"Question":[{"name":"example.com.","type":1}]}"#;
        assert_eq!(parse_response(negative, "example.com", 1), Ok((None, 0)));
        let ipv6 = br#"{"Status":0,"TC":false,"Question":[{"name":"example.com.","type":28}],"Answer":[{"name":"example.com.","type":28,"TTL":30,"data":"2606:4700:4700::1111"}]}"#;
        assert_eq!(
            parse_response(ipv6, "example.com", 28),
            Ok((Some("2606:4700:4700::1111".parse().unwrap()), 30))
        );
    }
}
