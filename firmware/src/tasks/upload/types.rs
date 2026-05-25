#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    BufferTooSmall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    MissingField,
    InvalidField,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseClass {
    Success,
    HttpFailure(u16),
    Malformed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointSource {
    Provisioned,
    Discovered,
    StaticFallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Endpoint {
    pub ipv4: [u8; 4],
    pub port: u16,
    pub source: EndpointSource,
}

impl Endpoint {
    pub const fn static_fallback() -> Self {
        Self {
            ipv4: config::upload::FALLBACK_IPV4_OCTETS,
            port: config::upload::FALLBACK_PORT,
            source: EndpointSource::StaticFallback,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndpointCandidates {
    pub provisioned: Option<Endpoint>,
    pub discovered: Option<Endpoint>,
    pub static_fallback: Endpoint,
}

impl EndpointCandidates {
    pub const fn fallback_only() -> Self {
        Self {
            provisioned: None,
            discovered: None,
            static_fallback: Endpoint::static_fallback(),
        }
    }
}

pub fn resolve_endpoint(candidates: EndpointCandidates) -> Endpoint {
    if let Some(endpoint) = candidates.provisioned {
        endpoint
    } else if let Some(endpoint) = candidates.discovered {
        endpoint
    } else {
        candidates.static_fallback
    }
}
