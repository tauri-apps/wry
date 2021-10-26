// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{convert::TryFrom, fmt};

use super::{
  header::{HeaderMap, HeaderName, HeaderValue},
  method::Method,
};

use crate::Result;

/// Represents an HTTP request from the WebView.
///
/// An HTTP request consists of a head and a potentially optional body.
///
/// ## Platform-specific
///
/// - **Linux:** Headers are not exposed.
pub struct Request {
  pub head: RequestParts,
  pub body: Vec<u8>,
}

/// Component parts of an HTTP `Request`
///
/// The HTTP request head consists of a method, uri, and a set of
/// header fields.
#[derive(Clone)]
pub struct RequestParts {
  /// The request's method
  pub method: Method,

  /// The request's URI
  pub uri: String,

  /// The request's headers
  pub headers: HeaderMap<HeaderValue>,
}

/// An HTTP request builder
///
/// This type can be used to construct an instance or `Request`
/// through a builder-like pattern.
#[derive(Debug)]
pub(crate) struct Builder {
  inner: Result<RequestParts>,
}

impl Request {
  /// Creates a new blank `Request` with the body
  #[inline]
  pub fn new(body: Vec<u8>) -> Request {
    Request {
      head: RequestParts::new(),
      body,
    }
  }

  /// Returns a reference to the associated HTTP method.
  #[inline]
  pub fn method(&self) -> &Method {
    &self.head.method
  }

  /// Returns a reference to the associated URI.
  #[inline]
  pub fn uri(&self) -> &str {
    &self.head.uri
  }

  /// Returns a reference to the associated header field map.
  #[inline]
  pub fn headers(&self) -> &HeaderMap<HeaderValue> {
    &self.head.headers
  }

  /// Returns a reference to the associated HTTP body.
  #[inline]
  pub fn body(&self) -> &Vec<u8> {
    &self.body
  }

  /// Consumes the request returning the head and body RequestParts.
  #[inline]
  pub fn into_parts(self) -> (RequestParts, Vec<u8>) {
    (self.head, self.body)
  }
}

impl Default for Request {
  fn default() -> Request {
    Request::new(Vec::new())
  }
}

impl fmt::Debug for Request {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Request")
      .field("method", self.method())
      .field("uri", &self.uri())
      .field("headers", self.headers())
      .field("body", self.body())
      .finish()
  }
}

impl RequestParts {
  /// Creates a new default instance of `RequestParts`
  fn new() -> RequestParts {
    RequestParts {
      method: Method::default(),
      uri: "".into(),
      headers: HeaderMap::default(),
    }
  }
}

impl fmt::Debug for RequestParts {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Parts")
      .field("method", &self.method)
      .field("uri", &self.uri)
      .field("headers", &self.headers)
      .finish()
  }
}

impl Builder {
  /// Creates a new default instance of `Builder` to construct a `Request`.
  #[inline]
  pub fn new() -> Builder {
    Builder::default()
  }

  /// Set the HTTP method for this request.
  ///
  /// This function will configure the HTTP method of the `Request` that will
  /// be returned from `Builder::build`.
  ///
  /// By default this is `GET`.
  pub fn method<T>(self, method: T) -> Builder
  where
    Method: TryFrom<T>,
    <Method as TryFrom<T>>::Error: Into<crate::Error>,
  {
    self.and_then(move |mut head| {
      let method = TryFrom::try_from(method).map_err(Into::into)?;
      head.method = method;
      Ok(head)
    })
  }

  /// Set the URI for this request.
  ///
  /// This function will configure the URI of the `Request` that will
  /// be returned from `Builder::build`.
  ///
  /// By default this is `/`.
  pub fn uri(self, uri: &str) -> Builder {
    self.and_then(move |mut head| {
      head.uri = uri.to_string();
      Ok(head)
    })
  }

  /// Appends a header to this request builder.
  ///
  /// This function will append the provided key/value as a header to the
  /// internal `HeaderMap` being constructed. Essentially this is equivalent
  /// to calling `HeaderMap::append`.
  #[allow(dead_code)] // It's not needed on Linux.
  pub fn header<K, V>(self, key: K, value: V) -> Builder
  where
    HeaderName: TryFrom<K>,
    <HeaderName as TryFrom<K>>::Error: Into<crate::Error>,
    HeaderValue: TryFrom<V>,
    <HeaderValue as TryFrom<V>>::Error: Into<crate::Error>,
  {
    self.and_then(move |mut head| {
      let name = <HeaderName as TryFrom<K>>::try_from(key).map_err(Into::into)?;
      let value = <HeaderValue as TryFrom<V>>::try_from(value).map_err(Into::into)?;
      head.headers.append(name, value);
      Ok(head)
    })
  }

  /// "Consumes" this builder, using the provided `body` to return a
  /// constructed `Request`.
  ///
  /// # Errors
  ///
  /// This function may return an error if any previously configured argument
  /// failed to parse or get converted to the internal representation. For
  /// example if an invalid `head` was specified via `header("Foo",
  /// "Bar\r\n")` the error will be returned when this function is called
  /// rather than when `header` was called.
  pub fn body(self, body: Vec<u8>) -> Result<Request> {
    self.inner.map(move |head| Request { head, body })
  }

  // private

  fn and_then<F>(self, func: F) -> Self
  where
    F: FnOnce(RequestParts) -> Result<RequestParts>,
  {
    Builder {
      inner: self.inner.and_then(func),
    }
  }
}

impl Default for Builder {
  #[inline]
  fn default() -> Builder {
    Builder {
      inner: Ok(RequestParts::new()),
    }
  }
}
