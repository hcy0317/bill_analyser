use std::net::{Ipv4Addr, Ipv6Addr};

use thiserror::Error;
use url::{Host, ParseError, Url};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboundHostClass {
    Public,
    Localhost,
    Metadata,
    Unspecified,
    Loopback,
    Private,
    LinkLocal,
    Multicast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum OutboundHttpUrlParseError {
    #[error("URL is empty")]
    Empty,
    #[error("URL contains unsafe text")]
    UnsafeText,
    #[error("URL is invalid")]
    Invalid,
    #[error("URL scheme is not HTTP or HTTPS")]
    UnsupportedScheme,
    #[error("URL contains credentials")]
    Credentials,
    #[error("URL host is missing")]
    MissingHost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundHttpUrl {
    url: Url,
    host_class: OutboundHostClass,
}

impl OutboundHttpUrl {
    pub fn parse(value: &str) -> Result<Self, OutboundHttpUrlParseError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(OutboundHttpUrlParseError::Empty);
        }
        if trimmed.contains('\\') || trimmed.chars().any(char::is_control) {
            return Err(OutboundHttpUrlParseError::UnsafeText);
        }
        let url = Url::parse(trimmed).map_err(|error| match error {
            ParseError::EmptyHost => OutboundHttpUrlParseError::MissingHost,
            _ => OutboundHttpUrlParseError::Invalid,
        })?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(OutboundHttpUrlParseError::UnsupportedScheme);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(OutboundHttpUrlParseError::Credentials);
        }
        let Some(host) = url.host() else {
            return Err(OutboundHttpUrlParseError::MissingHost);
        };
        let host_class = classify_host(host);
        Ok(Self { url, host_class })
    }

    pub fn as_url(&self) -> &Url {
        &self.url
    }

    pub fn into_url(self) -> Url {
        self.url
    }

    pub fn host_class(&self) -> OutboundHostClass {
        self.host_class
    }

    pub fn is_https(&self) -> bool {
        self.url.scheme() == "https"
    }

    pub fn same_origin(&self, other_url: &str) -> bool {
        Url::parse(other_url)
            .map(|other| self.url.origin() == other.origin())
            .unwrap_or(false)
    }

    pub fn matches_exact_or_origin_allowlist(&self, allowlist: &str) -> bool {
        let origin = self.url.origin().ascii_serialization().to_ascii_lowercase();
        let full = self.url.as_str().trim_end_matches('/').to_ascii_lowercase();
        allowlist
            .split([',', ';'])
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.trim_end_matches('/').to_ascii_lowercase())
            .any(|entry| entry == full || entry == origin)
    }

    pub fn has_query_or_fragment(&self) -> bool {
        self.url.query().is_some() || self.url.fragment().is_some()
    }
}

fn classify_host(host: Host<&str>) -> OutboundHostClass {
    match host {
        Host::Domain(host) => {
            let host = host.trim_end_matches('.');
            if host.eq_ignore_ascii_case("metadata.google.internal") {
                OutboundHostClass::Metadata
            } else if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
                OutboundHostClass::Localhost
            } else {
                OutboundHostClass::Public
            }
        }
        Host::Ipv4(address) => classify_ipv4(address),
        Host::Ipv6(address) => classify_ipv6(address),
    }
}

fn classify_ipv4(address: Ipv4Addr) -> OutboundHostClass {
    if matches!(
        address.octets(),
        [169, 254, 169, 254] | [100, 100, 100, 200]
    ) {
        OutboundHostClass::Metadata
    } else if address.is_unspecified() {
        OutboundHostClass::Unspecified
    } else if address.is_loopback() {
        OutboundHostClass::Loopback
    } else if address.is_multicast() {
        OutboundHostClass::Multicast
    } else if address.is_private() {
        OutboundHostClass::Private
    } else if address.is_link_local() {
        OutboundHostClass::LinkLocal
    } else {
        OutboundHostClass::Public
    }
}

fn classify_ipv6(address: Ipv6Addr) -> OutboundHostClass {
    if let Some(address) = address.to_ipv4_mapped() {
        classify_ipv4(address)
    } else if address.is_unspecified() {
        OutboundHostClass::Unspecified
    } else if address.is_loopback() {
        OutboundHostClass::Loopback
    } else if address.is_multicast() {
        OutboundHostClass::Multicast
    } else if address.is_unique_local() {
        OutboundHostClass::Private
    } else if address.is_unicast_link_local() {
        OutboundHostClass::LinkLocal
    } else {
        OutboundHostClass::Public
    }
}
