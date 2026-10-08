//! - WebView2 supports non-standard protocols only on Windows 10+, so we have to use a workaround.
//!   See <https://github.com/MicrosoftEdge/WebView2Feedback/issues/73>
//! - On Android, there's no API for registering custom protocols, so this workaround is also used.
//!
//! The process looks like this:
//!
//! 1. Use [`apply_uri_work_around`] to convert the URI we want to navigate to
//! 2. Intercept http(s) requests, test the request URI against [`is_work_around_uri`],
//!    if it matches, we apply [`revert_uri_work_around`] to the URI and feed it to the custom protocol handler
//!
//! On Android, a protocol mapped with `WebViewBuilderExtAndroid::with_custom_protocol_host` is served
//! from exactly `https://{host}` instead, using [`apply_mapped_host`] and [`match_mapped_host`].

use std::borrow::Cow;

use http::{
  Response, StatusCode, Uri,
  header::{CACHE_CONTROL, CONTENT_TYPE},
  uri::Scheme,
};

/// If the URI is a work around URI for this protocol which starts with `{http_or_https}://{protocol}.`
pub fn is_work_around_uri(uri: &str, http_or_https: &str, protocol: &str) -> bool {
  uri
    .strip_prefix(http_or_https)
    .and_then(|rest| rest.strip_prefix("://"))
    .and_then(|rest| rest.strip_prefix(protocol))
    .and_then(|rest| rest.strip_prefix("."))
    .is_some()
}

/// Converting `{protocol}://localhost/abc` to `{http_or_https}://{protocol}.localhost/abc`
pub fn apply_uri_work_around(uri: &str, http_or_https: &str, protocol: &str) -> String {
  uri.replace(
    &original_uri_prefix(protocol),
    &work_around_uri_prefix(http_or_https, protocol),
  )
}

/// Converting `{http_or_https}://{protocol}.localhost/abc` back to `{protocol}://localhost/abc`
pub fn revert_uri_work_around(uri: &str, http_or_https: &str, protocol: &str) -> String {
  uri.replace(
    &work_around_uri_prefix(http_or_https, protocol),
    &original_uri_prefix(protocol),
  )
}

pub fn original_uri_prefix(protocol: &str) -> String {
  format!("{protocol}://")
}

pub fn work_around_uri_prefix(http_or_https: &str, protocol: &str) -> String {
  format!("{http_or_https}://{protocol}.")
}

#[derive(Debug, PartialEq, Eq)]
pub enum MappedHost<'a> {
  /// `https://{host}` on the default port, which `protocol` serves as `canonical`.
  Serve {
    protocol: &'a str,
    canonical: Uri,
  },
  /// Any other form of a mapped host, e.g. `http`, another port, userinfo or a trailing dot.
  Block,
  NoMatch,
}

/// Routes `uri` against validated `(protocol, host)` mappings.
pub fn match_mapped_host<'a>(uri: &Uri, mappings: &'a [(String, String)]) -> MappedHost<'a> {
  let Some(authority) = uri.authority() else {
    return MappedHost::NoMatch;
  };
  let host = authority.host().trim_end_matches('.');
  let Some((protocol, mapped_host)) = mappings
    .iter()
    .find(|(_, mapped_host)| mapped_host.eq_ignore_ascii_case(host))
  else {
    return MappedHost::NoMatch;
  };

  let authority = authority.as_str();
  let authority = authority.strip_suffix(":443").unwrap_or(authority);
  if uri.scheme() != Some(&Scheme::HTTPS) || !authority.eq_ignore_ascii_case(mapped_host) {
    return MappedHost::Block;
  }

  match canonical_uri(protocol, uri) {
    Some(canonical) => MappedHost::Serve {
      protocol,
      canonical,
    },
    None => MappedHost::Block,
  }
}

fn canonical_uri(protocol: &str, uri: &Uri) -> Option<Uri> {
  let path = uri.path();
  let path_and_query = match uri.query() {
    Some(query) => format!("{path}?{query}"),
    None => path.to_string(),
  };
  Uri::builder()
    .scheme(protocol)
    .authority("localhost")
    .path_and_query(path_and_query)
    .build()
    .ok()
}

/// Converting `{protocol}://localhost/abc` to `https://{host}/abc`, or `None` for any other authority
pub fn apply_mapped_host(uri: &str, protocol: &str, host: &str) -> Option<String> {
  let rest = uri.strip_prefix(protocol)?.strip_prefix("://")?;
  let (authority, tail) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
  authority
    .eq_ignore_ascii_case("localhost")
    .then(|| format!("https://{host}{tail}"))
}

pub fn validate_mappings(
  mappings: &[(String, String)],
  is_registered: impl Fn(&str) -> bool,
) -> Result<(), String> {
  for (i, (protocol, host)) in mappings.iter().enumerate() {
    validate_host(host).map_err(|reason| format!("`{host}` for protocol `{protocol}` {reason}"))?;
    if !is_registered(protocol) || Scheme::try_from(protocol.as_str()).is_err() {
      return Err(format!(
        "`{protocol}` must be a registered custom protocol to be mapped to `{host}`"
      ));
    }
    let earlier = &mappings[..i];
    if earlier.iter().any(|(mapped, _)| mapped == protocol) {
      return Err(format!(
        "custom protocol `{protocol}` is already mapped to a host"
      ));
    }
    if let Some((mapped, _)) = earlier.iter().find(|(_, mapped_host)| mapped_host == host) {
      return Err(format!(
        "`{host}` is already mapped to custom protocol `{mapped}`"
      ));
    }
  }
  Ok(())
}

fn validate_host(host: &str) -> Result<(), &'static str> {
  if host.len() > 253 {
    return Err("must not be longer than 253 characters");
  }
  if host.ends_with('.') {
    return Err("must not end with a dot");
  }
  if host == "localhost" || host.ends_with(".localhost") {
    return Err("must not be `localhost` or a subdomain of it");
  }
  let Some((_, last_label)) = host.rsplit_once('.') else {
    return Err("must have at least two labels, like `app.example.com`");
  };
  for label in host.split('.') {
    if label.is_empty() || label.len() > 63 {
      return Err("must have labels of 1 to 63 characters");
    }
    if !label
      .bytes()
      .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
      return Err(
        "must only contain lowercase ASCII letters, digits, hyphens and dots, without a scheme, port or path",
      );
    }
    if label.starts_with('-') || label.ends_with('-') {
      return Err("must not have labels starting or ending with a hyphen");
    }
  }
  if ends_in_number(last_label) {
    return Err("must not be an IP address");
  }
  Ok(())
}

/// Whether URL parsers treat a host whose last label is `label` as an IPv4 address,
/// see <https://url.spec.whatwg.org/#ends-in-a-number-checker>.
fn ends_in_number(label: &str) -> bool {
  label.bytes().all(|b| b.is_ascii_digit())
    || label
      .strip_prefix("0x")
      .is_some_and(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub fn forbidden_response() -> Response<Cow<'static, [u8]>> {
  Response::builder()
    .status(StatusCode::FORBIDDEN)
    .header(CACHE_CONTROL, "no-store")
    .header(CONTENT_TYPE, "text/plain")
    .body(Cow::Borrowed(&[][..]))
    .unwrap()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn checks_if_custom_protocol_uri() {
    let scheme = "http";
    let uri = "http://wry.localhost/path/to/page";
    assert!(is_work_around_uri(uri, scheme, "wry"));
    assert!(!is_work_around_uri(uri, scheme, "asset"));
  }

  fn mapping(protocol: &str, host: &str) -> (String, String) {
    (protocol.to_string(), host.to_string())
  }

  fn route<'a>(uri: &str, mappings: &'a [(String, String)]) -> MappedHost<'a> {
    match_mapped_host(&uri.parse().unwrap(), mappings)
  }

  #[test]
  fn serves_exact_https_host() {
    let mappings = [mapping("tauri", "app.example.com")];
    for (uri, canonical) in [
      ("https://app.example.com", "tauri://localhost/"),
      ("https://app.example.com/", "tauri://localhost/"),
      (
        "https://app.example.com/assets/index.js",
        "tauri://localhost/assets/index.js",
      ),
      (
        "https://app.example.com/a%20b/c?d=e&f",
        "tauri://localhost/a%20b/c?d=e&f",
      ),
      ("https://app.example.com?b=c", "tauri://localhost/?b=c"),
      ("https://app.example.com/a?", "tauri://localhost/a?"),
      ("https://app.example.com:443/a", "tauri://localhost/a"),
      ("https://APP.Example.COM/a", "tauri://localhost/a"),
      ("HTTPS://app.example.com/a", "tauri://localhost/a"),
      (
        "https://app.example.com/a?r=https://app.example.com/b&s=https%3A%2F%2Fapp.example.com",
        "tauri://localhost/a?r=https://app.example.com/b&s=https%3A%2F%2Fapp.example.com",
      ),
      (
        "https://app.example.com/https://app.example.com/",
        "tauri://localhost/https://app.example.com/",
      ),
    ] {
      match route(uri, &mappings) {
        MappedHost::Serve {
          protocol: "tauri",
          canonical: served,
        } => assert_eq!(served.to_string(), canonical, "{uri}"),
        other => panic!("{uri} routed to {other:?}"),
      }
    }
  }

  #[test]
  fn blocks_other_forms_of_mapped_host() {
    let mappings = [mapping("tauri", "app.example.com")];
    for uri in [
      "http://app.example.com/",
      "http://app.example.com:443/",
      "http://app.example.com:8443/",
      "https://app.example.com:8443/",
      "https://app.example.com:80/",
      "https://app.example.com:0443/",
      "https://user@app.example.com/",
      "https://user:pass@app.example.com/",
      "https://app.example.com@app.example.com/",
      "https://app.example.com./",
      "https://app.example.com.:443/",
      "https://APP.EXAMPLE.COM./",
      "wss://app.example.com/",
      "ftp://app.example.com/",
    ] {
      assert_eq!(route(uri, &mappings), MappedHost::Block, "{uri}");
    }
  }

  #[test]
  fn ignores_lookalike_hosts() {
    let mappings = [mapping("tauri", "app.example.com")];
    for uri in [
      "https://app.example.com.evil.net/",
      "https://app.example.company/",
      "https://sub.app.example.com/",
      "https://example.com/",
      "https://pp.example.com/",
      "https://app-example.com/",
      "https://appxexample.com/",
      "https://app.example.com@evil.net/",
      "https://evil.net/?next=https://app.example.com/",
      "https://evil.net/app.example.com",
      "https://tauri.localhost/",
      "http://tauri.localhost/",
      "/relative/app.example.com",
    ] {
      assert_eq!(route(uri, &mappings), MappedHost::NoMatch, "{uri}");
    }
    for uri in ["https://app.example.com/", "http://wry.localhost/path"] {
      assert_eq!(route(uri, &[]), MappedHost::NoMatch, "{uri}");
    }
  }

  #[test]
  fn routes_each_host_to_its_protocol() {
    let mappings = [
      mapping("tauri", "app.example.com"),
      mapping("assets", "static.example.com"),
    ];
    assert_eq!(
      route("https://static.example.com/a", &mappings),
      MappedHost::Serve {
        protocol: "assets",
        canonical: "assets://localhost/a".parse().unwrap()
      }
    );
    assert_eq!(
      route("https://app.example.com/a", &mappings),
      MappedHost::Serve {
        protocol: "tauri",
        canonical: "tauri://localhost/a".parse().unwrap()
      }
    );
    assert_eq!(
      route("http://static.example.com/a", &mappings),
      MappedHost::Block
    );
  }

  #[test]
  fn applies_mapped_host_to_localhost_urls() {
    let host = "app.example.com";
    for (uri, mapped) in [
      ("tauri://localhost", "https://app.example.com"),
      ("tauri://localhost/", "https://app.example.com/"),
      (
        "tauri://localhost/index.html?q=1#/route",
        "https://app.example.com/index.html?q=1#/route",
      ),
      ("tauri://LocalHost/a", "https://app.example.com/a"),
      (
        "tauri://localhost?q=tauri://localhost/",
        "https://app.example.com?q=tauri://localhost/",
      ),
      ("tauri://localhost#x", "https://app.example.com#x"),
    ] {
      assert_eq!(
        apply_mapped_host(uri, "tauri", host).as_deref(),
        Some(mapped),
        "{uri}"
      );
    }
    for uri in [
      "tauri://localhost:8080/",
      "tauri://user@localhost/",
      "tauri://localhost.evil.net/",
      "tauri://localhostx/",
      "tauri://example.com/localhost",
      "wry://localhost/",
      "https://localhost/",
      "tauri:localhost/",
    ] {
      assert_eq!(apply_mapped_host(uri, "tauri", host), None, "{uri}");
    }
  }

  #[test]
  fn validates_hosts() {
    let longest = format!(
      "{}.{}.{}.{}",
      "a".repeat(63),
      "b".repeat(63),
      "c".repeat(63),
      "d".repeat(61)
    );
    assert_eq!(longest.len(), 253);

    for host in [
      "app.example.com",
      "a.b",
      "xn--bcher-kva.example",
      "app-1.example.com",
      "1app.example.com",
      "app.example.co.uk",
      &format!("{}.com", "a".repeat(63)),
      &longest,
    ] {
      assert_eq!(validate_host(host), Ok(()), "{host}");
    }

    for host in [
      "",
      ".",
      "com",
      "localhost",
      "app.localhost",
      "a.b.localhost",
      "App.example.com",
      "app.example.com.",
      "app..example.com",
      ".app.example.com",
      "-app.example.com",
      "app-.example.com",
      "app_x.example.com",
      "*.example.com",
      "app.example.com:443",
      "https://app.example.com/",
      "app.example.com/",
      "user@app.example.com",
      "bücher.example",
      "127.0.0.1",
      "1.2.3",
      "app.123",
      "app.example.0x7f",
      "app.example.0x",
      "[::1]",
      "::1",
      &format!("{}.com", "a".repeat(64)),
      &format!("{longest}a"),
    ] {
      assert!(validate_host(host).is_err(), "{host}");
    }
  }

  #[test]
  fn validates_mappings() {
    let registered = |protocol: &str| ["tauri", "assets"].contains(&protocol);

    assert_eq!(
      validate_mappings(
        &[
          mapping("tauri", "app.example.com"),
          mapping("assets", "static.example.com")
        ],
        registered
      ),
      Ok(())
    );

    for mappings in [
      vec![mapping("other", "app.example.com")],
      vec![mapping("tauri", "localhost")],
      vec![
        mapping("tauri", "app.example.com"),
        mapping("tauri", "other.example.com"),
      ],
      vec![
        mapping("tauri", "app.example.com"),
        mapping("assets", "app.example.com"),
      ],
    ] {
      assert!(
        validate_mappings(&mappings, registered).is_err(),
        "{mappings:?}"
      );
    }

    assert!(validate_mappings(&[mapping("not a scheme", "app.example.com")], |_| true).is_err());
  }

  #[test]
  fn forbidden_response_is_empty_and_not_cached() {
    let response = forbidden_response();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()[CACHE_CONTROL], "no-store");
    assert!(response.body().is_empty());
  }
}
