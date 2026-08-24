use bill_analyser_core::{OutboundHostClass, OutboundHttpUrl, OutboundHttpUrlParseError};

#[test]
fn outbound_http_url_normalizes_origin_and_allowlist_matching() {
    let endpoint =
        OutboundHttpUrl::parse(" https://Example.COM:443/v1/ ").expect("valid HTTPS endpoint");

    assert!(endpoint.is_https());
    assert_eq!(endpoint.host_class(), OutboundHostClass::Public);
    assert!(endpoint.same_origin("https://example.com/api"));
    assert!(endpoint
        .matches_exact_or_origin_allowlist("https://unrelated.example; https://example.com"));
    assert!(endpoint.matches_exact_or_origin_allowlist("https://example.com/v1"));
    assert!(!endpoint.matches_exact_or_origin_allowlist("https://example.com/other"));
}

#[test]
fn outbound_http_url_classifies_ssrf_relevant_hosts() {
    for (raw, expected) in [
        ("http://localhost:11434", OutboundHostClass::Localhost),
        ("http://localhost.", OutboundHostClass::Localhost),
        ("http://service.localhost", OutboundHostClass::Localhost),
        ("http://127.0.0.1", OutboundHostClass::Loopback),
        ("http://[::ffff:127.0.0.1]", OutboundHostClass::Loopback),
        ("http://[::1]", OutboundHostClass::Loopback),
        ("http://10.0.0.8", OutboundHostClass::Private),
        ("http://[fc00::1]", OutboundHostClass::Private),
        ("http://169.254.169.254", OutboundHostClass::Metadata),
        (
            "http://[::ffff:169.254.169.254]",
            OutboundHostClass::Metadata,
        ),
        ("http://100.100.100.200", OutboundHostClass::Metadata),
        ("http://169.254.10.8", OutboundHostClass::LinkLocal),
        ("http://[fe80::1]", OutboundHostClass::LinkLocal),
        ("http://0.0.0.0", OutboundHostClass::Unspecified),
        ("http://224.0.0.1", OutboundHostClass::Multicast),
        (
            "https://metadata.google.internal",
            OutboundHostClass::Metadata,
        ),
        (
            "https://metadata.google.internal.",
            OutboundHostClass::Metadata,
        ),
        ("https://api.openai.com/v1", OutboundHostClass::Public),
    ] {
        let endpoint = OutboundHttpUrl::parse(raw).expect(raw);
        assert_eq!(endpoint.host_class(), expected, "{raw}");
    }
}

#[test]
fn outbound_http_url_rejects_unsafe_authority_and_text_shapes() {
    for (raw, expected) in [
        ("", OutboundHttpUrlParseError::Empty),
        ("https:\\example.com", OutboundHttpUrlParseError::UnsafeText),
        (
            "https://example.com\npath",
            OutboundHttpUrlParseError::UnsafeText,
        ),
        ("not a URL", OutboundHttpUrlParseError::Invalid),
        (
            "ftp://example.com/file",
            OutboundHttpUrlParseError::UnsupportedScheme,
        ),
        (
            "https://user:secret@example.com",
            OutboundHttpUrlParseError::Credentials,
        ),
        ("https://?query", OutboundHttpUrlParseError::MissingHost),
    ] {
        assert_eq!(OutboundHttpUrl::parse(raw), Err(expected), "{raw}");
    }
}
