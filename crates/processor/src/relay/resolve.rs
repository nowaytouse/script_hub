//! Explicitly invoked, bounded DNS-over-HTTPS resolution.
use super::model::{
    dns,
    pipeline::{AddressFamily, ResolveError},
};
mod http;
mod wire;
use http::HttpTransport;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroize;

const LIMIT: usize = 1024;
const BUDGET: Duration = Duration::from_secs(120);

#[derive(Clone)]
struct Cached {
    value: wire::Answer,
    expires: Instant,
}
struct State {
    cache: HashMap<(String, u16, dns::Provider, dns::Format, bool), Cached>,
    requests: usize,
}
impl Drop for State {
    fn drop(&mut self) {
        for ((mut name, _, _, _, _), _) in self.cache.drain() {
            name.zeroize();
        }
    }
}
static COOLDOWN: Mutex<[Option<(Instant, u64)>; 2]> = Mutex::new([None, None]);
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
    age: u64,
    no_cache: bool,
    max_age: Option<u64>,
}
trait Transport: Send + Sync {
    fn get(
        &self,
        name: &str,
        kind: u16,
        options: &dns::Options,
        timeout: Duration,
    ) -> Result<Response, ResolveError>;
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
        Self::with_transport(cancel, Arc::new(HttpTransport::new()))
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
        options: &dns::Options,
    ) -> Result<dns::Batch, ResolveError> {
        dns::validate(options, true).map_err(|_| ResolveError::InvalidOptions)?;
        let include = if options.include.is_empty() {
            None
        } else {
            Some(regex::Regex::new(&options.include).map_err(|_| ResolveError::InvalidOptions)?)
        };
        let exclude = if options.exclude.is_empty() {
            None
        } else {
            Some(regex::Regex::new(&options.exclude).map_err(|_| ResolveError::InvalidOptions)?)
        };
        let report = Mutex::new(dns::Report::default());
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
        let mut all_domains = HashSet::new();
        let mut filtered_domains = HashSet::new();
        for host in hosts {
            let domain = canonical(host).filter(|_| host.parse::<IpAddr>().is_err());
            if let Some(name) = &domain {
                all_domains.insert(name.clone());
            }
            let match_target = domain.as_deref().unwrap_or(host);
            if include.as_ref().is_some_and(|r| !r.is_match(match_target))
                || exclude.as_ref().is_some_and(|r| r.is_match(match_target))
            {
                if let Some(name) = domain {
                    filtered_domains.insert(name);
                }
                literals.push(Some(None));
                mapping.push(0);
                continue;
            }
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
        {
            let mut stats = report.lock().map_err(|_| ResolveError::Unavailable)?;
            stats.unique_hosts = all_domains.len();
            stats.filtered = filtered_domains.len();
        }
        let results = Mutex::new(vec![None; unique.len()]);
        let next = AtomicUsize::new(0);
        let failure = Mutex::new(None);
        thread::scope(|scope| {
            for _ in 0..unique.len().min(options.concurrency) {
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
                            match self.query(name, kind, deadline, &failure, options, &report) {
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
        let unique_resolved = results
            .iter()
            .filter(|entry| matches!(entry, Some(Some(_))))
            .count();
        let addresses: Vec<_> = mapping
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
            .collect::<Result<_, _>>()?;
        let mut report = report.into_inner().map_err(|_| ResolveError::Unavailable)?;
        report.resolved = unique_resolved;
        Ok(dns::Batch { addresses, report })
    }
    fn query(
        &self,
        name: &str,
        kind: u16,
        deadline: Instant,
        failure: &Mutex<Option<ResolveError>>,
        options: &dns::Options,
        report: &Mutex<dns::Report>,
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
                .get(&(
                    name.to_owned(),
                    kind,
                    options.provider,
                    options.format,
                    options.require_ad,
                ))
                .filter(|_| options.cache)
                .filter(|e| e.expires > now)
            {
                let mut stats = report.lock().map_err(|_| ResolveError::Unavailable)?;
                stats.cache_hits += 1;
                if entry.value.negative {
                    stats.negative_hits += 1;
                }
                return Ok(select(&entry.value, name, options, &mut stats));
            }
            let provider_index = match options.provider {
                dns::Provider::Cloudflare => 0,
                dns::Provider::Google => 1,
            };
            if let Some((start, seconds)) =
                COOLDOWN.lock().map_err(|_| ResolveError::Unavailable)?[provider_index]
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
        let provider_index = match options.provider {
            dns::Provider::Cloudflare => 0,
            dns::Provider::Google => 1,
        };
        if let Some((start, seconds)) =
            COOLDOWN.lock().map_err(|_| ResolveError::Unavailable)?[provider_index]
        {
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
        let response = self.transport.get(
            name,
            kind,
            options,
            remaining.min(Duration::from_secs(options.timeout_seconds)),
        )?;
        report
            .lock()
            .map_err(|_| ResolveError::Unavailable)?
            .queries += 1;
        if response.status == 429 || (response.status != 200 && response.retry_after.is_some()) {
            let seconds = retry_after(response.retry_after.as_deref());
            let mut cooldown = COOLDOWN.lock().map_err(|_| ResolveError::Unavailable)?;
            let now = Instant::now();
            let old = cooldown[provider_index]
                .map(|(start, duration)| {
                    duration.saturating_sub(now.saturating_duration_since(start).as_secs())
                })
                .unwrap_or(0);
            cooldown[provider_index] = Some((now, seconds.max(old)));
            return Err(ResolveError::RateLimited {
                retry_after_seconds: seconds,
            });
        }
        if response.status != 200 {
            return Err(ResolveError::Unavailable);
        }
        let value = match options.format {
            dns::Format::Json => parse_json(&response.body, name, kind, options.require_ad)?,
            _ => wire::parse(&response.body, name, kind, options.require_ad)?,
        };
        let ttl = u64::from(value.ttl)
            .min(response.max_age.unwrap_or(u64::MAX))
            .saturating_sub(response.age);
        if ttl > 0 && options.cache && !response.no_cache {
            self.state
                .lock()
                .map_err(|_| ResolveError::Unavailable)?
                .cache
                .insert(
                    (
                        name.to_owned(),
                        kind,
                        options.provider,
                        options.format,
                        options.require_ad,
                    ),
                    Cached {
                        value: value.clone(),
                        expires: Instant::now() + Duration::from_secs(ttl.min(86400)),
                    },
                );
        }
        let mut stats = report.lock().map_err(|_| ResolveError::Unavailable)?;
        Ok(select(&value, name, options, &mut stats))
    }
}

fn retry_after(value: Option<&str>) -> u64 {
    let Some(value) = value else {
        return 60;
    };
    if let Some(seconds) = super::parse_retry_delay_seconds(value) {
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
    #[serde(rename = "AD", default)]
    authenticated: bool,
    #[serde(rename = "CD", default)]
    checking_disabled: bool,
    #[serde(rename = "Question")]
    question: Vec<Question>,
    #[serde(rename = "Answer", default)]
    answer: Vec<Record>,
    #[serde(rename = "Authority", default)]
    authority: Vec<Record>,
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
fn parse_json(
    bytes: &[u8],
    host: &str,
    kind: u16,
    require_ad: bool,
) -> Result<wire::Answer, ResolveError> {
    let reply: Reply = serde_json::from_slice(bytes).map_err(|_| ResolveError::InvalidResponse)?;
    if reply.truncated
        || reply.checking_disabled
        || reply.question.len() != 1
        || canonical(&reply.question[0].name).as_deref() != Some(host)
        || reply.question[0].kind != kind
    {
        return Err(ResolveError::InvalidResponse);
    }
    if require_ad && !reply.authenticated {
        return Err(ResolveError::Dnssec);
    }
    if reply.status != 0 && reply.status != 3 {
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
    let mut unsafe_answers = 0;
    for record in &reply.answer {
        let owner = canonical(&record.name).ok_or(ResolveError::InvalidResponse)?;
        if record.kind == 46 {
            if !names.contains(&owner) {
                return Err(ResolveError::InvalidResponse);
            }
            continue;
        }
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
        } else {
            unsafe_answers += 1;
        }
    }
    if reply.status == 3 && (!found.is_empty() || unsafe_answers > 0) {
        return Err(ResolveError::InvalidResponse);
    }
    let negative = found.is_empty() && unsafe_answers == 0;
    if negative {
        let mut soa_ttl: Option<u64> = None;
        for record in &reply.authority {
            if record.kind != 6 {
                continue;
            }
            let owner = if record.name == "." {
                String::new()
            } else {
                canonical(&record.name).ok_or(ResolveError::InvalidResponse)?
            };
            if !owner.is_empty() && current != owner && !current.ends_with(&format!(".{owner}")) {
                continue;
            }
            let fields: Vec<_> = record.data.split_ascii_whitespace().collect();
            if fields.len() != 7 {
                return Err(ResolveError::InvalidResponse);
            }
            let minimum: u64 = fields[6]
                .parse()
                .map_err(|_| ResolveError::InvalidResponse)?;
            let value = record.ttl.min(minimum);
            soa_ttl = Some(soa_ttl.map_or(value, |old| old.min(value)));
        }
        ttl = soa_ttl.map_or(0, |value| ttl.min(value));
    }
    Ok(wire::Answer {
        addresses: found,
        unsafe_answers,
        ttl: if ttl == u64::MAX {
            0
        } else {
            ttl.min(u32::MAX as u64) as u32
        },
        negative,
        nxdomain: reply.status == 3,
        authenticated: reply.authenticated,
    })
}

fn select(
    answer: &wire::Answer,
    name: &str,
    options: &dns::Options,
    report: &mut dns::Report,
) -> Option<IpAddr> {
    if answer.negative {
        if answer.nxdomain {
            report.nxdomain += 1;
        } else {
            report.nodata += 1;
        }
    }
    if answer.authenticated {
        report.authenticated += 1;
    } else {
        report.unverified += 1;
    }
    report.unsafe_answers += answer.unsafe_answers;
    report.candidates += answer.addresses.len();
    match options.selection {
        dns::Selection::First => answer.addresses.first().copied(),
        dns::Selection::Lowest => answer.addresses.iter().min().copied(),
        dns::Selection::Stable => {
            let hash = name
                .bytes()
                .chain(options.seed.to_le_bytes())
                .fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                });
            let mut ordered = answer.addresses.clone();
            ordered.sort_unstable();
            ordered.dedup();
            if ordered.is_empty() {
                None
            } else {
                Some(ordered[(hash % ordered.len() as u64) as usize])
            }
        }
    }
}
pub(super) fn public(ip: IpAddr) -> bool {
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
                || (o[0] == 192 && o[1] == 88 && o[2] == 99)
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19 || (o[1] == 51 && o[2] == 100)))
                || (o[0] == 203 && o[1] == 0 && o[2] == 113))
        }
        IpAddr::V6(v) => {
            let segments = v.segments();
            // Limit to global-unicast allocation, rejecting transition and
            // special-purpose ranges whose routing may embed another address.
            segments[0] & 0xe000 == 0x2000
                && !(segments[0] == 0x2001 && segments[1] <= 0x01ff)
                && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
                && segments[0] != 0x2002
                && !(segments[0] == 0x3fff && segments[1] <= 0x0fff)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn public_address_policy_rejects_special_and_embedded_ranges() {
        for address in [
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.1.1",
            "192.0.2.1",
            "192.88.99.1",
            "198.18.0.1",
            "203.0.113.1",
            "224.0.0.1",
            "::",
            "::1",
            "::a00:1",
            "::ffff:10.0.0.1",
            "64:ff9b::a00:1",
            "64:ff9b:1::a00:1",
            "2002:a00:1::",
            "2001:db8::1",
            "2001::1",
            "3fff::1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "4000::1",
        ] {
            assert!(!public(address.parse().unwrap()), "{address}");
        }
        for address in [
            "1.1.1.1",
            "8.8.8.8",
            "192.0.0.9",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
        ] {
            assert!(public(address.parse().unwrap()), "{address}");
        }
    }

    struct TestSession<'a>(Session<'a>);
    impl<'a> TestSession<'a> {
        fn with_transport(cancel: &'a AtomicBool, transport: Arc<dyn Transport>) -> Self {
            Self(Session::with_transport(cancel, transport))
        }
        fn resolve(
            &self,
            hosts: &[&str],
            family: AddressFamily,
        ) -> Result<Vec<Option<IpAddr>>, ResolveError> {
            let options = dns::Options {
                format: dns::Format::Json,
                ..Default::default()
            };
            self.0.resolve(hosts, family, &options).map(|b| b.addresses)
        }
    }
    fn parse_response(
        bytes: &[u8],
        host: &str,
        kind: u16,
    ) -> Result<(Option<IpAddr>, u64), ResolveError> {
        let result = parse_json(bytes, host, kind, false)?;
        Ok((result.addresses.iter().min().copied(), result.ttl.into()))
    }

    struct Fixture {
        calls: AtomicUsize,
        active: AtomicUsize,
        peak: AtomicUsize,
        mode: AtomicUsize,
        age: AtomicUsize,
        no_cache: AtomicBool,
        cancel: Arc<AtomicBool>,
    }
    impl Fixture {
        fn new(cancel: Arc<AtomicBool>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                active: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                mode: AtomicUsize::new(0),
                age: AtomicUsize::new(0),
                no_cache: AtomicBool::new(false),
                cancel,
            }
        }
    }
    impl Transport for Fixture {
        fn get(
            &self,
            name: &str,
            kind: u16,
            options: &dns::Options,
            _: Duration,
        ) -> Result<Response, ResolveError> {
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
                    age: 0,
                    no_cache: false,
                    max_age: None,
                });
            }
            if mode == 3 {
                return Ok(Response {
                    status: 503,
                    retry_after: Some("120".into()),
                    body: vec![],
                    age: 0,
                    no_cache: false,
                    max_age: None,
                });
            }
            if mode == 4 {
                return Ok(Response {
                    status: 503,
                    retry_after: Some(
                        (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc2822(),
                    ),
                    body: vec![],
                    age: 0,
                    no_cache: false,
                    max_age: None,
                });
            }
            let data = if kind == 1 {
                "1.1.1.1"
            } else {
                "2606:4700:4700::1111"
            };
            if options.format != dns::Format::Json {
                use hickory_proto::{
                    op::{Message, OpCode, Query},
                    rr::{
                        Name, RData, Record, RecordType,
                        rdata::{A, AAAA},
                    },
                };
                let owner = Name::from_ascii(format!("{name}.")).unwrap();
                let mut message = Message::response(0, OpCode::Query);
                message.add_query(Query::query(owner.clone(), RecordType::from(kind)));
                let data = if kind == 1 {
                    RData::A(A(data.parse().unwrap()))
                } else {
                    RData::AAAA(AAAA(data.parse().unwrap()))
                };
                message.answers.push(Record::from_rdata(owner, 1, data));
                return Ok(Response {
                    status: 200,
                    retry_after: None,
                    age: self.age.load(Ordering::SeqCst) as u64,
                    no_cache: self.no_cache.load(Ordering::SeqCst),
                    max_age: None,
                    body: message.to_vec().unwrap(),
                });
            }
            Ok(Response { status: 200, retry_after: None, age: self.age.load(Ordering::SeqCst) as u64, no_cache: self.no_cache.load(Ordering::SeqCst), max_age: None, body: serde_json::json!({"Status": 0, "TC": false, "Question": [{"name": name, "type": kind}], "Answer": [{"name": name, "type": kind, "TTL": 1, "data": data}]}).to_string().into_bytes() })
        }
    }

    #[test]
    fn injected_transport_covers_batch_cache_cancel_and_cooldown() {
        let cancel = Arc::new(AtomicBool::new(false));
        let fixture = Arc::new(Fixture::new(cancel.clone()));
        let session = TestSession::with_transport(&cancel, fixture.clone());
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
        let scoped = dns::Options {
            format: dns::Format::Json,
            include: "^d\\.".into(),
            ..Default::default()
        };
        let batch = session
            .0
            .resolve(
                &[
                    "d.example",
                    "D.EXAMPLE.",
                    "excluded.example",
                    "EXCLUDED.EXAMPLE.",
                ],
                AddressFamily::V4,
                &scoped,
            )
            .unwrap();
        assert_eq!(batch.report.unique_hosts, 2);
        assert_eq!(batch.report.filtered, 1);
        assert_eq!(batch.report.resolved, 1);
        assert_eq!(batch.report.cache_hits, 1);
        assert_eq!(batch.report.candidates, 1);
        let before = fixture.calls.load(Ordering::SeqCst);
        session.resolve(&["d.example"], AddressFamily::V4).unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), before);
        thread::sleep(Duration::from_millis(1100));
        session.resolve(&["d.example"], AddressFamily::V4).unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), before + 1);
        let google = dns::Options {
            provider: dns::Provider::Google,
            format: dns::Format::Json,
            ..Default::default()
        };
        let calls = fixture.calls.load(Ordering::SeqCst);
        session
            .0
            .resolve(&["d.example"], AddressFamily::V4, &google)
            .unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), calls + 1);
        fixture.age.store(1, Ordering::SeqCst);
        session
            .0
            .resolve(&["age.example"], AddressFamily::V4, &google)
            .unwrap();
        let calls = fixture.calls.load(Ordering::SeqCst);
        session
            .0
            .resolve(&["age.example"], AddressFamily::V4, &google)
            .unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), calls + 1);
        fixture.age.store(0, Ordering::SeqCst);
        fixture.no_cache.store(true, Ordering::SeqCst);
        session
            .0
            .resolve(&["no-cache.example"], AddressFamily::V4, &google)
            .unwrap();
        let calls = fixture.calls.load(Ordering::SeqCst);
        session
            .0
            .resolve(&["no-cache.example"], AddressFamily::V4, &google)
            .unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), calls + 1);
        fixture.no_cache.store(false, Ordering::SeqCst);
        for format in [dns::Format::Post, dns::Format::Get] {
            let wire = dns::Options {
                format,
                cache: false,
                ..google.clone()
            };
            assert_eq!(
                session
                    .0
                    .resolve(&["wire.example"], AddressFamily::V4, &wire)
                    .unwrap()
                    .addresses,
                vec![Some("1.1.1.1".parse().unwrap())]
            );
        }
        let bypass = dns::Options {
            cache: false,
            ..google.clone()
        };
        let calls = fixture.calls.load(Ordering::SeqCst);
        session
            .0
            .resolve(&["d.example"], AddressFamily::V4, &bypass)
            .unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), calls + 1);
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
        let another = TestSession::with_transport(&cancel, fixture.clone());
        let calls = fixture.calls.load(Ordering::SeqCst);
        assert!(matches!(
            another.resolve(&["other.example"], AddressFamily::V4),
            Err(ResolveError::RateLimited {
                retry_after_seconds: 7199..=7200
            })
        ));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), calls);
        *COOLDOWN.lock().unwrap() = [None, None];
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
        *COOLDOWN.lock().unwrap() = [None, None];
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
        *COOLDOWN.lock().unwrap() = [None, None];
    }

    #[test]
    fn parses_retry_after_without_shortening_server_minimum() {
        assert_eq!(retry_after(Some("7200")), 7200);
        assert_eq!(retry_after(Some("18446744073709551616")), u64::MAX);
        assert_eq!(retry_after(Some("not-a-date")), 60);
        let future = (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc2822();
        assert!(retry_after(Some(&future)) >= 119);
    }

    #[test]
    fn expired_session_never_dispatches() {
        let cancel = Arc::new(AtomicBool::new(false));
        let fixture = Arc::new(Fixture::new(cancel.clone()));
        let mut session = TestSession::with_transport(&cancel, fixture.clone());
        session.0.deadline = Instant::now() - Duration::from_secs(1);
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

    #[test]
    fn selection_preserves_first_and_stable_ignores_order() {
        let mut answer = wire::Answer {
            addresses: vec!["8.8.8.8".parse().unwrap(), "1.1.1.1".parse().unwrap()],
            unsafe_answers: 0,
            ttl: 10,
            negative: false,
            nxdomain: false,
            authenticated: false,
        };
        let mut options = dns::Options {
            selection: dns::Selection::First,
            ..Default::default()
        };
        assert_eq!(
            select(
                &answer,
                "example.com",
                &options,
                &mut dns::Report::default()
            ),
            Some("8.8.8.8".parse().unwrap())
        );
        options.selection = dns::Selection::Stable;
        let selected = select(
            &answer,
            "example.com",
            &options,
            &mut dns::Report::default(),
        );
        answer.addresses.reverse();
        assert_eq!(
            selected,
            select(
                &answer,
                "example.com",
                &options,
                &mut dns::Report::default()
            )
        );
    }

    #[test]
    #[ignore = "opt-in public DNS integration; requires network"]
    fn public_doh_smoke_for_example_domains() {
        let cancel = AtomicBool::new(false);
        let session = Session::new(&cancel);
        for provider in [dns::Provider::Cloudflare, dns::Provider::Google] {
            for format in [dns::Format::Post, dns::Format::Get, dns::Format::Json] {
                let options = dns::Options {
                    provider,
                    format,
                    cache: false,
                    ..Default::default()
                };
                let batch = session
                    .resolve(&["example.com", "iana.org"], AddressFamily::V4, &options)
                    .unwrap();
                assert!(batch.addresses.iter().all(Option::is_some));
            }
        }
    }
}
