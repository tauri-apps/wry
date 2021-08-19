use std::{any::Any, convert::TryFrom, fmt};

use super::{
  header::{HeaderMap, HeaderName, HeaderValue},
  method::Method,
  version::Version,
  Extensions, Uri,
};
use crate::Result;

pub struct Request {
  head: Parts,
  body: Vec<u8>,
}

/// Component parts of an HTTP `Request`
///
/// The HTTP request head consists of a method, uri, version, and a set of
/// header fields.
pub struct Parts {
  /// The request's method
  pub method: Method,

  /// The request's URI
  pub uri: Uri,

  /// The request's version
  pub version: Version,

  /// The request's headers
  pub headers: HeaderMap<HeaderValue>,

  /// The request's extensions
  pub extensions: Extensions,

  _priv: (),
}

/// An HTTP request builder
///
/// This type can be used to construct an instance or `Request`
/// through a builder-like pattern.
#[derive(Debug)]
pub struct Builder {
  inner: Result<Parts>,
}

impl Request {
  /// Creates a new blank `Request` with the body
  #[inline]
  pub fn new(body: Vec<u8>) -> Request {
    Request {
      head: Parts::new(),
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
  pub fn uri(&self) -> &Uri {
    &self.head.uri
  }

  /// Returns the associated version.
  #[inline]
  pub fn version(&self) -> Version {
    self.head.version
  }

  /// Returns a reference to the associated header field map.
  #[inline]
  pub fn headers(&self) -> &HeaderMap<HeaderValue> {
    &self.head.headers
  }

  /// Returns a reference to the associated extensions.
  #[inline]
  pub fn extensions(&self) -> &Extensions {
    &self.head.extensions
  }
  /// Returns a reference to the associated HTTP body.
  #[inline]
  pub fn body(&self) -> &Vec<u8> {
    &self.body
  }

  /// Consumes the request returning the head and body parts.
  #[inline]
  pub fn into_parts(self) -> (Parts, Vec<u8>) {
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
      .field("uri", self.uri())
      .field("version", &self.version())
      .field("headers", self.headers())
      // omits Extensions because not useful
      .field("body", self.body())
      .finish()
  }
}

impl Parts {
  /// Creates a new default instance of `Parts`
  fn new() -> Parts {
    Parts {
      method: Method::default(),
      uri: Uri::default(),
      version: Version::default(),
      headers: HeaderMap::default(),
      extensions: Extensions::default(),
      _priv: (),
    }
  }
}

impl fmt::Debug for Parts {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Parts")
      .field("method", &self.method)
      .field("uri", &self.uri)
      .field("version", &self.version)
      .field("headers", &self.headers)
      // omits Extensions because not useful
      // omits _priv because not useful
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

  /// Get the HTTP Method for this request.
  ///
  /// By default this is `GET`. If builder has error, returns None.
  pub fn method_ref(&self) -> Option<&Method> {
    self.inner.as_ref().ok().map(|h| &h.method)
  }

  /// Set the URI for this request.
  ///
  /// This function will configure the URI of the `Request` that will
  /// be returned from `Builder::build`.
  ///
  /// By default this is `/`.
  pub fn uri<T>(self, uri: T) -> Builder
  where
    Uri: TryFrom<T>,
    <Uri as TryFrom<T>>::Error: Into<crate::Error>,
  {
    self.and_then(move |mut head| {
      head.uri = TryFrom::try_from(uri).map_err(Into::into)?;
      Ok(head)
    })
  }

  /// Get the URI for this request
  pub fn uri_ref(&self) -> Option<&Uri> {
    self.inner.as_ref().ok().map(|h| &h.uri)
  }

  /// Set the HTTP version for this request.
  ///
  /// This function will configure the HTTP version of the `Request` that
  /// will be returned from `Builder::build`.
  ///
  /// By default this is HTTP/1.1
  pub fn version(self, version: Version) -> Builder {
    self.and_then(move |mut head| {
      head.version = version;
      Ok(head)
    })
  }

  /// Appends a header to this request builder.
  ///
  /// This function will append the provided key/value as a header to the
  /// internal `HeaderMap` being constructed. Essentially this is equivalent
  /// to calling `HeaderMap::append`.
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

  /// Get header on this request builder.
  /// when builder has error returns None
  pub fn headers_ref(&self) -> Option<&HeaderMap<HeaderValue>> {
    self.inner.as_ref().ok().map(|h| &h.headers)
  }

  /// Get headers on this request builder.
  ///
  /// When builder has error returns None.
  pub fn headers_mut(&mut self) -> Option<&mut HeaderMap<HeaderValue>> {
    self.inner.as_mut().ok().map(|h| &mut h.headers)
  }

  /// Adds an extension to this builder
  pub fn extension<T>(self, extension: T) -> Builder
  where
    T: Any + Send + Sync + 'static,
  {
    self.and_then(move |mut head| {
      head.extensions.insert(extension);
      Ok(head)
    })
  }

  /// Get a reference to the extensions for this request builder.
  ///
  /// If the builder has an error, this returns `None`.
  pub fn extensions_ref(&self) -> Option<&Extensions> {
    self.inner.as_ref().ok().map(|h| &h.extensions)
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
    F: FnOnce(Parts) -> Result<Parts>,
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
      inner: Ok(Parts::new()),
    }
  }
}
