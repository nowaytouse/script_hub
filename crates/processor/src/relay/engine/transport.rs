use std::{
    fmt,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use ureq::unversioned::{
    resolver::{ResolvedSocketAddrs, Resolver},
    transport::{
        ConnectionDetails, Connector, NextTimeout, RustlsConnector, TcpConnector, TransportAdapter,
        time::{Duration as UDuration, Instant as UInstant},
    },
};
use zeroize::Zeroizing;

use super::EngineError;

#[derive(Debug)]
struct LoopbackResolver;
impl Resolver for LoopbackResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        _: &ureq::config::Config,
        _: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        if uri.host() != Some("127.0.0.1") {
            return Err(failed(TunnelFailure::Local));
        }
        let port = uri.port_u16().ok_or_else(|| failed(TunnelFailure::Local))?;
        let mut addresses = self.empty();
        addresses.push(SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
        Ok(addresses)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TunnelFailure {
    Local,
    Protocol,
    Remote(u8),
}

impl fmt::Display for TunnelFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SOCKS tunnel failed: {self:?}")
    }
}
impl std::error::Error for TunnelFailure {}

pub(super) fn failure(error: &ureq::Error) -> Option<TunnelFailure> {
    match error {
        ureq::Error::Other(error) => error.downcast_ref::<TunnelFailure>().copied(),
        _ => None,
    }
}

fn failed(error: TunnelFailure) -> ureq::Error {
    ureq::Error::Other(Box::new(error))
}

struct Socks {
    port: u16,
    user: Zeroizing<String>,
    password: Zeroizing<String>,
}
impl fmt::Debug for Socks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Socks")
    }
}

fn handshake(
    w: &mut (impl Read + Write),
    user: &str,
    password: &str,
    host: &str,
    port: u16,
) -> Result<(), TunnelFailure> {
    let io = |_| TunnelFailure::Protocol;
    w.write_all(&[5, 1, 2]).map_err(io)?;
    let mut pair = [0; 2];
    w.read_exact(&mut pair).map_err(io)?;
    if pair != [5, 2] {
        return Err(TunnelFailure::Local);
    }
    let mut auth = Zeroizing::new(vec![1, user.len() as u8]);
    auth.extend_from_slice(user.as_bytes());
    auth.push(password.len() as u8);
    auth.extend_from_slice(password.as_bytes());
    w.write_all(&auth).map_err(io)?;
    w.read_exact(&mut pair).map_err(io)?;
    if pair != [1, 0] {
        return Err(TunnelFailure::Local);
    }
    let mut request = vec![5, 1, 0, 3, host.len() as u8];
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(&port.to_be_bytes());
    w.write_all(&request).map_err(io)?;
    let mut header = [0; 4];
    w.read_exact(&mut header).map_err(io)?;
    if header[0] != 5 || header[2] != 0 {
        return Err(TunnelFailure::Protocol);
    }
    let len = match header[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut len = [0];
            w.read_exact(&mut len).map_err(io)?;
            if len[0] == 0 {
                return Err(TunnelFailure::Protocol);
            }
            usize::from(len[0])
        }
        _ => return Err(TunnelFailure::Protocol),
    };
    let mut address = [0; 257];
    w.read_exact(&mut address[..len + 2]).map_err(io)?;
    match header[1] {
        0 => Ok(()),
        code @ 1..=8 => Err(TunnelFailure::Remote(code)),
        _ => Err(TunnelFailure::Protocol),
    }
}

impl Connector for Socks {
    type Out = <TcpConnector as Connector<()>>::Out;
    fn connect(
        &self,
        details: &ConnectionDetails,
        _: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        if !details.needs_tls() {
            return Err(failed(TunnelFailure::Local));
        }
        let host = details
            .uri
            .host()
            .ok_or_else(|| failed(TunnelFailure::Local))?;
        if host.is_empty() || host.len() > 255 || host.contains(':') {
            return Err(failed(TunnelFailure::Local));
        }
        let target_port = details.uri.port_u16().unwrap_or(443);
        let mut addrs = details.resolver.empty();
        addrs.push(SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)));
        let local = ConnectionDetails {
            addrs,
            uri: details.uri,
            config: details.config,
            request_level: details.request_level,
            resolver: details.resolver,
            now: details.now,
            timeout: details.timeout,
            current_time: details.current_time.clone(),
            run_connector: details.run_connector.clone(),
            register_shutdown: details.register_shutdown.clone(),
        };
        let transport = TcpConnector::default()
            .connect(&local, None)
            .map_err(|_| failed(TunnelFailure::Local))?
            .ok_or_else(|| failed(TunnelFailure::Local))?;
        let mut timeout = details.timeout;
        if let (UDuration::Exact(limit), UInstant::Exact(start), UInstant::Exact(now)) =
            (timeout.after, details.now, (details.current_time)())
        {
            timeout.after = limit
                .saturating_sub(now.saturating_duration_since(start))
                .into();
        }
        if matches!(timeout.after, UDuration::Exact(value) if value.is_zero()) {
            return Err(failed(TunnelFailure::Local));
        }
        let mut stream = TransportAdapter::new(transport, details.current_time.clone());
        stream.set_timeout(timeout);
        handshake(&mut stream, &self.user, &self.password, host, target_port).map_err(failed)?;
        Ok(Some(stream.into_inner()))
    }
}

pub(super) fn agent(
    port: u16,
    user: &str,
    password: &str,
    timeout: Duration,
) -> Result<ureq::Agent, EngineError> {
    if port == 0 || !(1..=255).contains(&user.len()) || !(1..=255).contains(&password.len()) {
        return Err(EngineError::InvalidOptions);
    }
    let proxy = ureq::Proxy::builder(ureq::ProxyProtocol::Socks5h)
        .host("127.0.0.1")
        .port(port)
        .resolve_target(false)
        .build()
        .map_err(|_| EngineError::Infrastructure)?;
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .max_idle_connections(0)
        .proxy(Some(proxy))
        .build();
    let connector = Socks {
        port,
        user: Zeroizing::new(user.to_owned()),
        password: Zeroizing::new(password.to_owned()),
    }
    .chain(RustlsConnector::default());
    Ok(ureq::Agent::with_parts(config, connector, LoopbackResolver))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread, time::Instant};

    fn server(reply: Vec<u8>, greeting: [u8; 2], auth: [u8; 2]) -> (u16, thread::JoinHandle<()>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut greeting_request = [0; 3];
            stream.read_exact(&mut greeting_request).unwrap();
            assert_eq!(greeting_request, [5, 1, 2]);
            stream.write_all(&greeting).unwrap();
            if greeting != [5, 2] {
                return;
            }
            let mut credentials = [0; 5];
            stream.read_exact(&mut credentials).unwrap();
            assert_eq!(credentials, [1, 1, b'u', 1, b'p']);
            stream.write_all(&auth).unwrap();
            if auth != [1, 0] {
                return;
            }
            let mut request = [0; 22];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request[..5], &[5, 1, 0, 3, 15]);
            assert_eq!(&request[5..20], b"example.invalid");
            stream.write_all(&reply).unwrap();
            if reply.get(1) == Some(&0) {
                let mut tls = [0; 5];
                stream.read_exact(&mut tls).unwrap();
                assert_eq!(
                    tls[0], 0x16,
                    "a successful SOCKS tunnel must start TLS, not plaintext HTTP"
                );
                assert_eq!(tls[1], 0x03);
            }
        });
        (port, handle)
    }

    fn run(reply: Vec<u8>, greeting: [u8; 2], auth: [u8; 2]) -> TunnelFailure {
        let (port, handle) = server(reply, greeting, auth);
        let error = agent(port, "u", "p", Duration::from_secs(2))
            .unwrap()
            .head("https://example.invalid/")
            .call()
            .unwrap_err();
        handle.join().unwrap();
        failure(&error).unwrap()
    }

    #[test]
    fn explicit_socks_replies_are_structured() {
        for code in 1..=8 {
            assert_eq!(
                run(vec![5, code, 0, 1, 0, 0, 0, 0, 0, 0], [5, 2], [1, 0]),
                TunnelFailure::Remote(code)
            );
        }
    }

    #[test]
    fn malformed_truncated_and_auth_refusal_are_not_remote_failures() {
        assert_eq!(
            run(vec![5, 5, 0, 1], [5, 2], [1, 0]),
            TunnelFailure::Protocol
        );
        assert_eq!(
            run(vec![4, 5, 0, 1, 0, 0, 0, 0, 0, 0], [5, 2], [1, 0]),
            TunnelFailure::Protocol
        );
        assert_eq!(run(vec![], [5, 0], [1, 0]), TunnelFailure::Local);
        assert_eq!(run(vec![], [5, 2], [1, 1]), TunnelFailure::Local);
    }

    #[test]
    fn handshake_timeout_is_bounded() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(500));
        });
        let started = Instant::now();
        let result = agent(port, "u", "p", Duration::from_millis(100))
            .unwrap()
            .head("https://example.invalid/")
            .call();
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        handle.join().unwrap();
    }

    #[test]
    fn local_connection_failure_is_not_remote_failure() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let error = agent(port, "u", "p", Duration::from_secs(1))
            .unwrap()
            .head("https://example.invalid/")
            .call()
            .unwrap_err();
        assert_eq!(failure(&error), Some(TunnelFailure::Local));
    }

    #[test]
    fn successful_socks_tunnel_still_requires_tls_and_never_reuses_connections() {
        let (port, worker) = server(vec![5, 0, 0, 1, 0, 0, 0, 0, 0, 0], [5, 2], [1, 0]);
        let client = agent(port, "u", "p", Duration::from_secs(2)).unwrap();
        assert_eq!(client.config().max_idle_connections(), 0);
        let error = client.head("https://example.invalid/").call().unwrap_err();
        worker.join().unwrap();
        assert!(failure(&error).is_none());
    }

    #[test]
    fn resolver_fails_closed_without_looking_up_a_target_hostname() {
        let config = ureq::Agent::config_builder().build();
        let timeout = NextTimeout::default();
        assert!(
            LoopbackResolver
                .resolve(
                    &"https://example.invalid/".parse().unwrap(),
                    &config,
                    timeout
                )
                .is_err()
        );
        let addresses = LoopbackResolver
            .resolve(
                &"socks5h://127.0.0.1:12345".parse().unwrap(),
                &config,
                timeout,
            )
            .unwrap();
        assert_eq!(
            &addresses[..],
            &["127.0.0.1:12345".parse::<SocketAddr>().unwrap()]
        );
    }
}
