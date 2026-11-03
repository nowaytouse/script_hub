use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::{Read, Write},
    net::{IpAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::model::{Health, ModelError, Subscription};
use serde_json::{Value, json};
use zeroize::{Zeroize, Zeroizing};

mod report;
mod transport;
use report::Outcome;
pub use report::{ProbeDetail, ProbeReason};

const TARGETS: [&str; 2] = [
    "https://www.gstatic.com/generate_204",
    "https://cp.cloudflare.com/generate_204",
];

pub struct ProbeOptions {
    pub engine: PathBuf,
    pub concurrency: usize,
    pub timeout: Duration,
    pub compatibility: bool,
    pub doh: bool,
}

impl Default for ProbeOptions {
    fn default() -> Self {
        Self {
            engine: PathBuf::from("sing-box"),
            concurrency: 4,
            timeout: Duration::from_secs(8),
            compatibility: false,
            doh: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineError {
    InvalidOptions,
    Unavailable,
    Cancelled,
    Infrastructure,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node check stopped: {self:?}")
    }
}
impl std::error::Error for EngineError {}

pub struct ProbeBatch {
    pub health: Vec<Health>,
    pub baseline_valid: usize,
    pub details: Vec<ProbeDetail>,
}

#[cfg(test)]
impl ProbeBatch {
    pub(crate) fn from_health(health: Vec<Health>, baseline_valid: usize) -> Self {
        let details = health
            .iter()
            .map(|health| {
                Outcome::new(
                    *health,
                    match health {
                        Health::Healthy { .. } => ProbeReason::Verified,
                        Health::Failed => ProbeReason::ConnectionFailed,
                        Health::Unknown => ProbeReason::Inconclusive,
                        Health::SecurityBlocked => ProbeReason::StrictTlsUnverified,
                    },
                )
                .detail
            })
            .collect();
        Self {
            health,
            baseline_valid,
            details,
        }
    }
}

enum Attempt {
    Success(u64),
    Transport,
    Connection,
    Rejected,
    Tls,
    Http,
    Policy,
}

fn classify(baseline: [bool; 2], attempts: &[Vec<Attempt>], alive: bool) -> Health {
    if !alive {
        return Health::Unknown;
    }
    if let Some(latency_ms) = attempts
        .iter()
        .flatten()
        .filter_map(|attempt| {
            if let Attempt::Success(ms) = attempt {
                Some(*ms)
            } else {
                None
            }
        })
        .min()
    {
        return Health::Healthy { latency_ms };
    }
    if baseline.iter().all(|valid| *valid)
        && attempts.len() == 2
        && attempts.iter().all(|group| {
            group.len() >= 2
                && group
                    .iter()
                    .all(|attempt| matches!(attempt, Attempt::Transport | Attempt::Connection))
        })
    {
        Health::Failed
    } else {
        Health::Unknown
    }
}

fn unsafe_address(server: &str) -> bool {
    let server = server.trim_matches(['[', ']']);
    let normalized = server.trim_end_matches('.').to_ascii_lowercase();
    if normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".local")
    {
        return true;
    }
    let Ok(address) = server.parse::<IpAddr>() else {
        return server.is_empty() || server.chars().any(char::is_whitespace);
    };
    match address {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.octets()[0] == 0
                || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|v4| unsafe_address(&v4.to_string()))
        }
    }
}

fn pin_endpoint(outbound: &Value, doh: bool, timeout: Duration) -> Option<Value> {
    use ureq::unversioned::{
        resolver::{DefaultResolver, Resolver},
        transport::{NextTimeout, time::Duration as UDuration},
    };
    let server = outbound.get("server")?.as_str()?;
    if unsafe_address(server) {
        return None;
    }
    let host = url::Host::parse(server.trim_matches(['[', ']'])).ok()?;
    let ip = match host {
        url::Host::Ipv4(ip) => IpAddr::V4(ip),
        url::Host::Ipv6(ip) => IpAddr::V6(ip),
        url::Host::Domain(ref name) => {
            // Avoid changing transport Host semantics and avoid silently replacing DoH with system DNS.
            if doh || outbound.get("transport").is_some() {
                return None;
            }
            let uri = format!("https://{name}/").parse().ok()?;
            let config = ureq::Agent::config_builder().proxy(None).build();
            let addresses = DefaultResolver::default()
                .resolve(
                    &uri,
                    &config,
                    NextTimeout {
                        after: UDuration::Exact(timeout),
                        ..Default::default()
                    },
                )
                .ok()?;
            select_public_address(addresses.iter().map(|address| address.ip()))?
        }
    };
    if !super::resolve::public(ip) {
        return None;
    }
    pin_address(outbound, ip)
}

fn select_public_address(addresses: impl IntoIterator<Item = IpAddr>) -> Option<IpAddr> {
    let mut addresses = addresses.into_iter();
    let first = addresses.next()?;
    if !super::resolve::public(first) || addresses.any(|ip| !super::resolve::public(ip)) {
        return None;
    }
    Some(first)
}

fn pin_address(outbound: &Value, ip: IpAddr) -> Option<Value> {
    if !super::resolve::public(ip) {
        return None;
    }
    let original = outbound.get("server")?.as_str()?;
    let mut pinned = outbound.clone();
    let object = pinned.as_object_mut()?;
    if let Some(tls) = object.get_mut("tls") {
        let tls = tls.as_object_mut()?;
        let name = match tls.get("server_name") {
            None => "",
            Some(Value::String(name)) => name.as_str(),
            Some(_) => return None,
        };
        if original.trim_matches(['[', ']']).parse::<IpAddr>().is_err()
            && tls.get("enabled") == Some(&Value::Bool(true))
            && name.is_empty()
        {
            tls.insert("server_name".into(), original.into());
        }
    }
    object.insert("server".into(), ip.to_string().into());
    Some(pinned)
}

fn contains_external_resource(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            let key = key.replace('-', "_").to_ascii_lowercase();
            key.ends_with("_path")
                || matches!(
                    key.as_str(),
                    "detour"
                        | "domain_resolver"
                        | "http_client"
                        | "bind_interface"
                        | "routing_mark"
                        | "plugin"
                        | "process"
                        | "command"
                        | "script"
                        | "inbounds"
                        | "route"
                        | "experimental"
                        | "services"
                )
                || contains_external_resource(value)
        }),
        Value::Array(values) => values.iter().any(contains_external_resource),
        _ => false,
    }
}

fn isolated_config(
    outbound: &Value,
    port: u16,
    user: &str,
    password: &str,
    doh: bool,
) -> Result<Value, EngineError> {
    if port == 0
        || user.is_empty()
        || password.is_empty()
        || !matches!(
            outbound.get("type").and_then(Value::as_str),
            Some(
                "anytls"
                    | "shadowsocks"
                    | "socks"
                    | "http"
                    | "trojan"
                    | "vmess"
                    | "vless"
                    | "tuic"
                    | "hysteria2"
            )
        )
        || contains_external_resource(outbound)
        || outbound
            .get("server")
            .and_then(Value::as_str)
            .is_none_or(unsafe_address)
    {
        return Err(EngineError::Infrastructure);
    }
    let mut node = outbound.clone();
    let Some(object) = node.as_object_mut() else {
        return Err(EngineError::Infrastructure);
    };
    object.insert("tag".to_owned(), Value::String("candidate".to_owned()));
    let dns = if doh {
        json!({"type":"https","tag":"bootstrap","server":"1.1.1.1","server_port":443,"path":"/dns-query","tls":{"enabled":true,"server_name":"cloudflare-dns.com"}})
    } else {
        json!({"type":"local","tag":"bootstrap"})
    };
    Ok(
        json!({"log":{"disabled":true},"dns":{"servers":[dns]},"inbounds":[{"type":"socks","tag":"probe-in","listen":"127.0.0.1","listen_port":port,"users":[{"username":user,"password":password}]}],"outbounds":[node],"route":{"final":"candidate","default_domain_resolver":"bootstrap"}}),
    )
}

fn wipe(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(values) => values.iter_mut().for_each(wipe),
        Value::Object(fields) => fields.values_mut().for_each(wipe),
        _ => {}
    }
}
struct SecretJson(Value);
impl Drop for SecretJson {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

struct Process {
    child: Child,
    writer: Option<JoinHandle<std::io::Result<()>>>,
    finished: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum ConfigStatus {
    Accepted,
    Rejected,
    TimedOut,
}

impl Process {
    fn spawn(engine: &Path, action: &str, config: &Value) -> Result<Self, EngineError> {
        let encoded =
            Zeroizing::new(serde_json::to_vec(config).map_err(|_| EngineError::Infrastructure)?);
        if encoded.len() > 1024 * 1024 {
            return Err(EngineError::Infrastructure);
        }
        let mut command = Command::new(engine);
        command
            .args([action, "-c", "stdin", "--disable-color"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env_clear();
        for name in [
            "SystemRoot",
            "WINDIR",
            "SystemDrive",
            "HOME",
            "USERPROFILE",
            "LOCALAPPDATA",
            "TEMP",
            "TMP",
            "TMPDIR",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let child = command.spawn().map_err(|_| EngineError::Unavailable)?;
        let mut process = Self {
            child,
            writer: None,
            finished: false,
        };
        let Some(mut input) = process.child.stdin.take() else {
            return Err(EngineError::Infrastructure);
        };
        process.writer = Some(thread::spawn(move || input.write_all(&encoded)));
        Ok(process)
    }
    fn alive(&mut self) -> Result<bool, EngineError> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|_| EngineError::Infrastructure)
    }
    fn finish(&mut self) -> Result<(), EngineError> {
        if self.finished {
            return Ok(());
        }
        if self.alive()? {
            self.child.kill().map_err(|_| EngineError::Infrastructure)?;
        }
        self.child.wait().map_err(|_| EngineError::Infrastructure)?;
        if let Some(writer) = self.writer.take() {
            writer
                .join()
                .map_err(|_| EngineError::Infrastructure)?
                .map_err(|_| EngineError::Infrastructure)?;
        }
        self.finished = true;
        Ok(())
    }
    fn checked(&mut self, cancel: &AtomicBool) -> Result<ConfigStatus, EngineError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            ensure_active(cancel)?;
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|_| EngineError::Infrastructure)?
            {
                self.finish()?;
                return Ok(if status.success() {
                    ConfigStatus::Accepted
                } else {
                    ConfigStatus::Rejected
                });
            }
            if Instant::now() >= deadline {
                self.finish()?;
                return Ok(ConfigStatus::TimedOut);
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn ready(
        &mut self,
        port: u16,
        user: &str,
        password: &str,
        cancel: &AtomicBool,
    ) -> Result<bool, EngineError> {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            ensure_active(cancel)?;
            if !self.alive()? {
                return Ok(false);
            }
            if listener_authenticated(port, user, password) {
                return self.alive();
            }
            thread::sleep(Duration::from_millis(30));
        }
        Ok(false)
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(writer) = self.writer.take() {
                let _ = writer.join();
            }
        }
    }
}

fn ensure_active(cancel: &AtomicBool) -> Result<(), EngineError> {
    if cancel.load(Ordering::Acquire) {
        Err(EngineError::Cancelled)
    } else {
        Ok(())
    }
}

fn executable(path: &Path) -> Result<PathBuf, EngineError> {
    let candidate = if path.is_absolute() || path.components().count() > 1 {
        path.canonicalize().map_err(|_| EngineError::Unavailable)?
    } else {
        let name = path.as_os_str().to_owned();
        #[cfg(windows)]
        let name = {
            let mut name = name;
            if path.extension().is_none() {
                name.push(".exe");
            }
            name
        };
        find_in_directories(
            &name,
            std::env::var_os("PATH")
                .into_iter()
                .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
                .chain({
                    #[cfg(target_os = "macos")]
                    {
                        vec![
                            PathBuf::from("/opt/homebrew/bin"),
                            PathBuf::from("/usr/local/bin"),
                        ]
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        Vec::<PathBuf>::new()
                    }
                }),
        )
        .ok_or(EngineError::Unavailable)?
    };
    if !candidate.is_file() {
        return Err(EngineError::Unavailable);
    }
    #[cfg(windows)]
    if !candidate
        .extension()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("exe"))
    {
        return Err(EngineError::Unavailable);
    }
    Ok(candidate)
}

fn find_in_directories(
    name: &std::ffi::OsStr,
    directories: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    directories
        .into_iter()
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
}

/// Locate an existing sing-box executable without launching it.
pub fn discover_engine() -> Option<PathBuf> {
    if let Ok(engine) = executable(Path::new("sing-box")) {
        return Some(engine);
    }
    #[cfg(windows)]
    {
        let directory = std::env::current_exe().ok()?.parent()?.to_path_buf();
        executable(&directory.join("sing-box.exe")).ok()
    }
    #[cfg(not(windows))]
    None
}

fn agent(proxy: Option<ureq::Proxy>, timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(0)
        .max_redirects_will_error(false)
        .max_idle_connections(0)
        .http_status_as_error(false)
        .proxy(proxy)
        .build()
        .into()
}

fn request(client: &ureq::Agent, url: &str) -> Attempt {
    let started = Instant::now();
    match client.head(url).config().https_only(true).build().call() {
        Ok(response) if response.status().as_u16() == 204 => {
            Attempt::Success(started.elapsed().as_millis().min(u64::MAX as u128) as u64)
        }
        Ok(_) => Attempt::Http,
        Err(error) => error_attempt(&error),
    }
}

fn error_attempt(error: &ureq::Error) -> Attempt {
    if let Some(failure) = transport::failure(error) {
        return match failure {
            transport::TunnelFailure::Remote(3..=6) => Attempt::Connection,
            transport::TunnelFailure::Remote(1 | 2) => Attempt::Rejected,
            _ => Attempt::Policy,
        };
    }
    if matches!(error, ureq::Error::Timeout(_))
        || matches!(error, ureq::Error::Io(error) if error.kind() == std::io::ErrorKind::TimedOut)
    {
        Attempt::Transport
    } else if matches!(error, ureq::Error::Tls(_) | ureq::Error::Rustls(_)) {
        Attempt::Tls
    } else {
        Attempt::Policy
    }
}

fn socks_auth(port: u16, user: &str, password: &str) -> Option<bool> {
    if !(1..=255).contains(&user.len()) || !(1..=255).contains(&password.len()) {
        return None;
    }
    let timeout = Duration::from_millis(100);
    let mut stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    stream.write_all(&[5, 1, 2]).ok()?;
    let mut reply = [0; 2];
    stream.read_exact(&mut reply).ok()?;
    if reply != [5, 2] {
        return None;
    }
    let mut auth = Zeroizing::new(vec![1, user.len() as u8]);
    auth.extend_from_slice(user.as_bytes());
    auth.push(password.len() as u8);
    auth.extend_from_slice(password.as_bytes());
    stream.write_all(&auth).ok()?;
    stream.read_exact(&mut reply).ok()?;
    (reply[0] == 1).then_some(reply[1] == 0)
}

fn listener_authenticated(port: u16, user: &str, password: &str) -> bool {
    // ureq also offers no-auth, so prove that this listener rejects the wrong password first.
    let Ok(wrong) = credential() else {
        return false;
    };
    socks_auth(port, user, &wrong) == Some(false) && socks_auth(port, user, password) == Some(true)
}

fn baseline(options: &ProbeOptions, cancel: &AtomicBool) -> Result<[bool; 2], EngineError> {
    let client = agent(None, options.timeout);
    let mut valid = [false; 2];
    for (index, target) in TARGETS.iter().enumerate() {
        ensure_active(cancel)?;
        valid[index] = matches!(request(&client, target), Attempt::Success(_));
    }
    Ok(valid)
}

fn credential() -> Result<Zeroizing<String>, EngineError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| EngineError::Infrastructure)?;
    Ok(Zeroizing::new(
        bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
    ))
}

fn probe_one(
    outbound: &Value,
    engine: &Path,
    options: &ProbeOptions,
    valid: [bool; 2],
    cancel: &AtomicBool,
) -> Result<Outcome, EngineError> {
    ensure_active(cancel)?;
    if outbound
        .get("server")
        .and_then(Value::as_str)
        .and_then(|server| server.trim_matches(['[', ']']).parse::<IpAddr>().ok())
        .is_some_and(|ip| !super::resolve::public(ip))
    {
        return Ok(Outcome::new(Health::Unknown, ProbeReason::UnsafeAddress));
    }
    if contains_external_resource(outbound) {
        return Ok(Outcome::new(Health::Unknown, ProbeReason::ExternalResource));
    }
    let Some(pinned) = pin_endpoint(outbound, options.doh, options.timeout) else {
        return Ok(Outcome::new(
            Health::Unknown,
            ProbeReason::AddressUnverified,
        ));
    };
    let pinned = SecretJson(pinned);
    let port = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map_err(|_| EngineError::Infrastructure)?
        .port();
    let user = credential()?;
    let password = credential()?;
    let Ok(config) = isolated_config(&pinned.0, port, &user, &password, options.doh) else {
        return Ok(Outcome::new(Health::Unknown, ProbeReason::Unsupported));
    };
    let config = SecretJson(config);
    let mut checker = Process::spawn(engine, "check", &config.0)?;
    match checker.checked(cancel)? {
        ConfigStatus::Accepted => {}
        ConfigStatus::Rejected => {
            return Ok(Outcome::new(
                Health::Unknown,
                ProbeReason::EngineConfigRejected,
            ));
        }
        ConfigStatus::TimedOut => {
            return Ok(Outcome::new(
                Health::Unknown,
                ProbeReason::EngineConfigTimeout,
            ));
        }
    }
    let mut process = Process::spawn(engine, "run", &config.0)?;
    if !process.ready(port, &user, &password, cancel)? {
        let reason = if process.alive()? {
            ProbeReason::EngineNotReady
        } else {
            ProbeReason::EngineExited
        };
        process.finish()?;
        return Ok(Outcome::new(Health::Unknown, reason));
    }
    let client = transport::agent(port, &user, &password, options.timeout)?;
    let result = probe_targets(
        valid,
        cancel,
        || process.alive(),
        |target| request(&client, target),
    )?;
    process.finish()?;
    Ok(result)
}

fn probe_targets(
    valid: [bool; 2],
    cancel: &AtomicBool,
    mut alive: impl FnMut() -> Result<bool, EngineError>,
    mut fetch: impl FnMut(&str) -> Attempt,
) -> Result<Outcome, EngineError> {
    let mut attempts = [Vec::new(), Vec::new()];
    let order = if !valid[0] && valid[1] {
        [1, 0]
    } else {
        [0, 1]
    };
    for _ in 0..2 {
        for index in order {
            ensure_active(cancel)?;
            if !alive()? {
                return Ok(Outcome::new(Health::Unknown, ProbeReason::EngineExited));
            }
            let attempt = fetch(TARGETS[index]);
            ensure_active(cancel)?;
            let success = matches!(attempt, Attempt::Success(_));
            attempts[index].push(attempt);
            if success {
                let live = alive()?;
                let mut result = Outcome::new(
                    classify(valid, &attempts, live),
                    if live {
                        ProbeReason::Verified
                    } else {
                        ProbeReason::EngineExited
                    },
                );
                if live {
                    result.detail.target = Some(index);
                }
                return Ok(result);
            }
        }
    }
    let live = alive()?;
    let health = classify(valid, &attempts, live);
    let reason = if !live {
        ProbeReason::EngineExited
    } else if attempts
        .iter()
        .flatten()
        .any(|a| matches!(a, Attempt::Http))
    {
        ProbeReason::HttpResponse
    } else if attempts.iter().flatten().any(|a| matches!(a, Attempt::Tls)) {
        ProbeReason::TlsFailure
    } else if attempts
        .iter()
        .flatten()
        .any(|a| matches!(a, Attempt::Rejected))
    {
        ProbeReason::TunnelRejected
    } else if attempts
        .iter()
        .flatten()
        .any(|a| matches!(a, Attempt::Policy))
    {
        ProbeReason::Inconclusive
    } else if !valid.iter().all(|v| *v) {
        ProbeReason::BaselineUnstable
    } else if attempts
        .iter()
        .flatten()
        .any(|a| matches!(a, Attempt::Connection))
    {
        ProbeReason::ConnectionFailed
    } else {
        ProbeReason::Timeout
    };
    Ok(Outcome::new(health, reason))
}

struct Measurements(Mutex<HashMap<String, Outcome>>);
fn measurement_key(outbound: &Value) -> Result<Zeroizing<String>, EngineError> {
    let mut identity = SecretJson(outbound.clone());
    identity
        .0
        .as_object_mut()
        .ok_or(EngineError::Infrastructure)?
        .remove("tag");
    serde_json::to_string(&identity.0)
        .map(Zeroizing::new)
        .map_err(|_| EngineError::Infrastructure)
}
impl Drop for Measurements {
    fn drop(&mut self) {
        let values = self
            .0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (mut key, _) in values.drain() {
            key.zeroize();
        }
    }
}
struct HostPermit<'a> {
    hosts: &'a Mutex<HashSet<String>>,
    name: String,
}
fn host_key(server: &str) -> String {
    let bare = server
        .strip_prefix('[')
        .and_then(|name| name.strip_suffix(']'))
        .unwrap_or(server)
        .trim_end_matches('.');
    match bare.parse::<IpAddr>() {
        Ok(address) => address.to_string(),
        Err(_) => bare.to_ascii_lowercase(),
    }
}

impl<'a> HostPermit<'a> {
    fn try_acquire(
        hosts: &'a Mutex<HashSet<String>>,
        server: &str,
    ) -> Result<Option<Self>, EngineError> {
        let name = host_key(server);
        let mut occupied = hosts.lock().map_err(|_| EngineError::Infrastructure)?;
        if occupied.insert(name.clone()) {
            Ok(Some(Self { hosts, name }))
        } else {
            Ok(None)
        }
    }
}
impl Drop for HostPermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut hosts) = self.hosts.lock()
            && let Some(mut name) = hosts.take(&self.name)
        {
            name.zeroize();
        }
        self.name.zeroize();
    }
}

pub fn check_nodes(
    subscription: &Subscription,
    options: &ProbeOptions,
    cancel: &AtomicBool,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<ProbeBatch, EngineError> {
    ensure_active(cancel)?;
    if !(1..=8).contains(&options.concurrency)
        || options.timeout < Duration::from_secs(1)
        || options.timeout > Duration::from_secs(20)
        || subscription.nodes().is_empty()
    {
        return Err(EngineError::InvalidOptions);
    }
    let engine = executable(&options.engine)?;
    let initial = baseline(options, cancel)?;
    let count = subscription.nodes().len();
    let next = AtomicUsize::new(0);
    let completed = AtomicUsize::new(0);
    let abort = AtomicBool::new(false);
    let hosts = Mutex::new(HashSet::new());
    let measurements = Measurements(Mutex::new(HashMap::new()));
    let mut health = vec![Health::Unknown; count];
    let mut details = vec![ProbeDetail::new(ProbeReason::Inconclusive); count];
    let result = thread::scope(|scope| -> Result<(), EngineError> {
        let mut handles = Vec::new();
        for _ in 0..options.concurrency.min(count) {
            handles.push(
                scope.spawn(|| -> Result<Vec<(usize, Outcome)>, EngineError> {
                    let result = (|| {
                        let mut outcomes = Vec::new();
                        loop {
                            ensure_active(cancel)?;
                            if abort.load(Ordering::Acquire) {
                                break;
                            }
                            let index = next.fetch_add(1, Ordering::AcqRel);
                            let Some(node) = subscription.nodes().get(index) else {
                                break;
                            };
                            let outcome = match node.to_probe_sing_box(options.compatibility) {
                                Err(ModelError::SecurityBlocked) => Outcome::new(
                                    Health::SecurityBlocked,
                                    ProbeReason::StrictTlsUnverified,
                                ),
                                Err(_) => Outcome::new(Health::Unknown, ProbeReason::Unsupported),
                                Ok((outbound, strict_tls)) => {
                                    let outbound = SecretJson(outbound);
                                    let key = measurement_key(&outbound.0)?;
                                    let permit = loop {
                                        ensure_active(cancel)?;
                                        if abort.load(Ordering::Acquire) {
                                            return Err(EngineError::Infrastructure);
                                        }
                                        if let Some(permit) =
                                            HostPermit::try_acquire(&hosts, node.server())?
                                        {
                                            break permit;
                                        }
                                        thread::sleep(Duration::from_millis(30));
                                    };
                                    let cached = measurements
                                        .0
                                        .lock()
                                        .map_err(|_| EngineError::Infrastructure)?
                                        .get(key.as_str())
                                        .copied();
                                    let value = match cached {
                                        Some(value) => value,
                                        None => {
                                            let value = probe_one(
                                                &outbound.0,
                                                &engine,
                                                options,
                                                initial,
                                                cancel,
                                            )?;
                                            measurements
                                                .0
                                                .lock()
                                                .map_err(|_| EngineError::Infrastructure)?
                                                .insert(key.to_string(), value);
                                            value
                                        }
                                    };
                                    drop(permit);
                                    value.strict_tls(strict_tls)
                                }
                            };
                            outcomes.push((index, outcome));
                            progress(completed.fetch_add(1, Ordering::AcqRel) + 1, count);
                        }
                        Ok(outcomes)
                    })();
                    if result.is_err() {
                        abort.store(true, Ordering::Release);
                    }
                    result
                }),
            );
        }
        let mut error = None;
        for handle in handles {
            match handle.join() {
                Ok(Ok(outcomes)) => {
                    for (index, outcome) in outcomes {
                        health[index] = outcome.health;
                        details[index] = outcome.detail;
                    }
                }
                Ok(Err(failure)) => {
                    abort.store(true, Ordering::Release);
                    error.get_or_insert(failure);
                }
                Err(_) => {
                    abort.store(true, Ordering::Release);
                    error.get_or_insert(EngineError::Infrastructure);
                }
            }
        }
        error.map_or(Ok(()), Err)
    });
    result?;
    ensure_active(cancel)?;
    let final_baseline = baseline(options, cancel)?;
    let valid = [
        initial[0] && final_baseline[0],
        initial[1] && final_baseline[1],
    ];
    if !valid.iter().all(|item| *item) {
        for (result, detail) in health.iter_mut().zip(&mut details) {
            if *result == Health::Failed {
                *result = Health::Unknown;
                detail.reason = ProbeReason::BaselineUnstable;
            }
        }
    }
    Ok(ProbeBatch {
        health,
        baseline_valid: valid.into_iter().filter(|valid| *valid).count(),
        details,
    })
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
