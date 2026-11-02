use super::Health;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeReason {
    Verified,
    Unsupported,
    UnsafeAddress,
    ExternalResource,
    EngineConfigRejected,
    EngineConfigTimeout,
    EngineNotReady,
    EngineExited,
    ConnectionFailed,
    TunnelRejected,
    Timeout,
    HttpResponse,
    TlsFailure,
    Inconclusive,
    StrictTlsUnverified,
    BaselineUnstable,
    NotTested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ProbeDetail {
    pub reason: ProbeReason,
    pub latency_ms: Option<u64>,
    pub target: Option<usize>,
    pub strict_tls: bool,
}

impl ProbeDetail {
    pub(crate) fn new(reason: ProbeReason) -> Self {
        Self {
            reason,
            latency_ms: None,
            target: None,
            strict_tls: false,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Outcome {
    pub health: Health,
    pub detail: ProbeDetail,
}

impl Outcome {
    pub fn new(health: Health, reason: ProbeReason) -> Self {
        let mut detail = ProbeDetail::new(reason);
        if let Health::Healthy { latency_ms } = health {
            detail.latency_ms = Some(latency_ms);
        }
        Self { health, detail }
    }

    pub fn strict_tls(mut self, strict: bool) -> Self {
        self.detail.strict_tls = strict;
        // Certificate/protocol ambiguity is not the same as an explicit network-unreachable reply.
        if strict
            && !matches!(self.health, Health::Healthy { .. })
            && matches!(
                self.detail.reason,
                ProbeReason::TunnelRejected
                    | ProbeReason::Timeout
                    | ProbeReason::HttpResponse
                    | ProbeReason::TlsFailure
                    | ProbeReason::Inconclusive
                    | ProbeReason::BaselineUnstable
            )
        {
            self.health = Health::SecurityBlocked;
        }
        self
    }
}
