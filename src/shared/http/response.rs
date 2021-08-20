use super::{
  header::{HeaderMap, HeaderName, HeaderValue},
  status::StatusCode,
  version::Version,
  Extensions,
};
use crate::Result;
use std::{any::Any, convert::TryFrom, fmt};

/// Represents an HTTP response
///
/// An HTTP response consists of a head and a potentially body.
///
/// ## Platform-specific
///
/// - **Linux:** Headers and status code cannot be changed.
///
/// # Examples
///
/// ```
/// # use wry::http::*;
///
/// let response = ResponseBuilder::new("text/html")
///     .status(202)
///     .body("hello!".as_bytes())
///     .unwrap();
/// ```
///
pub struct Response {
  head: Parts,
  body: Vec<u8>,
  mimetype: String,
}

/// Component parts of an HTTP `Response`
///
/// The HTTP response head consists of a status, version, and a set of
/// header fields.
#[non_exhaustive]
pub struct Parts {
  /// The response's status
  pub status: StatusCode,

  /// The response's version
  pub version: Version,

  /// The response's headers
  pub headers: HeaderMap<HeaderValue>,

  /// The response's extensions
  pub extensions: Extensions,
}

/// An HTTP response builder
///
/// This type can be used to construct an instance of `Response` through a
/// builder-like pattern.
#[derive(Debug)]
pub struct Builder {
  inner: Result<Parts>,
  mimetype: String,
}

impl Response {
  /// Creates a new blank `Response` with the body
  #[inline]
  pub fn new(body: Vec<u8>) -> Response {
    Response {
      head: Parts::new(),
      mimetype: "application/octet-stream".to_string(),
      body,
    }
  }

  /// Returns the `StatusCode`.
  #[inline]
  pub fn status(&self) -> StatusCode {
    self.head.status
  }

  /// Returns a reference to the mime type.
  #[inline]
  pub fn mimetype(&self) -> &str {
    &self.mimetype
  }

  /// Returns a reference to the associated version.
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
}

impl Default for Response {
  #[inline]
  fn default() -> Response {
    Response::new(Vec::new())
  }
}

impl fmt::Debug for Response {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Response")
      .field("status", &self.status())
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
      status: StatusCode::default(),
      version: Version::default(),
      headers: HeaderMap::default(),
      extensions: Extensions::default(),
    }
  }
}

impl fmt::Debug for Parts {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Parts")
      .field("status", &self.status)
      .field("version", &self.version)
      .field("headers", &self.headers)
      // omits Extensions because not useful
      .finish()
  }
}

impl Builder {
  /// Creates a new default instance of `Builder` to construct either a
  /// `Head` or a `Response`.
  ///
  /// # Examples
  ///
  /// ```
  /// # use wry::http::*;
  ///
  /// let response = ResponseBuilder::new("text/html")
  ///     .status(200)
  ///     .body(Vec::new())
  ///     .unwrap();
  /// ```
  #[inline]
  pub fn new(mimetype: &str) -> Builder {
    Builder {
      inner: Ok(Parts::new()),
      mimetype: mimetype.to_string(),
    }
  }

  /// Set the HTTP status for this response.
  pub fn status<T>(self, status: T) -> Builder
  where
    StatusCode: TryFrom<T>,
    <StatusCode as TryFrom<T>>::Error: Into<crate::Error>,
  {
    self.and_then(move |mut head| {
      head.status = TryFrom::try_from(status).map_err(Into::into)?;
      Ok(head)
    })
  }

  /// Set the HTTP version for this response.
  ///
  /// This function will configure the HTTP version of the `Response` that
  /// will be returned from `Builder::build`.
  ///
  /// By default this is HTTP/1.1
  pub fn version(self, version: Version) -> Builder {
    self.and_then(move |mut head| {
      head.version = version;
      Ok(head)
    })
  }

  /// Appends a header to this response builder.
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

  /// "Consumes" this builder, using the provided `body` to return a
  /// constructed `Response`.
  ///
  /// # Errors
  ///
  /// This function may return an error if any previously configured argument
  /// failed to parse or get converted to the internal representation. For
  /// example if an invalid `head` was specified via `header("Foo",
  /// "Bar\r\n")` the error will be returned when this function is called
  /// rather than when `header` was called.
  ///
  /// # Examples
  ///
  /// ```
  /// # use wry::http::*;
  ///
  /// let response = ResponseBuilder::new("text/html")
  ///     .body(Vec::new())
  ///     .unwrap();
  /// ```
  pub fn body(self, body: Vec<u8>) -> Result<Response> {
    let mimetype = self.mimetype;
    self.inner.map(move |head| Response {
      mimetype,
      head,
      body,
    })
  }

  // private

  fn and_then<F>(self, func: F) -> Self
  where
    F: FnOnce(Parts) -> Result<Parts>,
  {
    Builder {
      mimetype: self.mimetype,
      inner: self.inner.and_then(func),
    }
  }
}

impl Default for Builder {
  #[inline]
  fn default() -> Builder {
    Builder {
      inner: Ok(Parts::new()),
      mimetype: "application/octet-stream".to_string(),
    }
  }
}
