/// Convenient type alias of Result type for wry.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by wry.
#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
  #[cfg(gtk)]
  #[error(transparent)]
  GlibError(#[from] webkit6::glib::Error),
  #[cfg(gtk)]
  #[error(transparent)]
  GlibBoolError(#[from] webkit6::glib::BoolError),
  #[cfg(gtk)]
  #[error("Fail to fetch security manager")]
  MissingManager,
  #[cfg(gtk)]
  #[error("Couldn't find X11 Display")]
  X11DisplayNotFound,
  #[cfg(gtk)]
  #[error(
    "Wayland window handles are not supported by the native X11 embedding path. \
     Use WebViewBuilderExtUnix::build_gtk to embed a WebView inside a Wayland application."
  )]
  WaylandNotSupported,
  #[cfg(gtk)]
  #[error(
    "No realized GTK4 window found that owns the given Wayland wl_surface. \
     The parent window must be a GTK4 toplevel created before the WebView is built — \
     surfaces from foreign toolkits (e.g. a bare winit window on Wayland) cannot be \
     matched. Use WebViewBuilderExtUnix::build_gtk to embed a webview into any GTK \
     widget without requiring the `wayland` feature flag."
  )]
  WaylandWindowNotFound,
  #[cfg(all(gtk, feature = "x11"))]
  #[error(transparent)]
  XlibError(#[from] x11_dl::error::OpenError),
  #[error("Failed to initialize the script")]
  InitScriptError,
  #[error("Bad RPC request: {0} ((1))")]
  RpcScriptError(String, String),
  #[error(transparent)]
  NulError(#[from] std::ffi::NulError),
  #[error(transparent)]
  ReceiverError(#[from] std::sync::mpsc::RecvError),
  #[cfg(target_os = "android")]
  #[error(transparent)]
  ReceiverTimeoutError(#[from] crossbeam_channel::RecvTimeoutError),
  #[error(transparent)]
  SenderError(#[from] std::sync::mpsc::SendError<String>),
  #[error("Failed to send the message")]
  MessageSender,
  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),
  #[cfg(target_os = "windows")]
  #[error("WebView2 error: {0}")]
  WebView2Error(webview2_com::Error),
  #[error(transparent)]
  HttpError(#[from] http::Error),
  #[error("Infallible error, something went really wrong: {0}")]
  Infallible(#[from] std::convert::Infallible),
  #[cfg(target_os = "android")]
  #[error(transparent)]
  JniError(#[from] jni::errors::Error),
  #[error("Failed to create proxy endpoint")]
  ProxyEndpointCreationFailed,
  #[error(transparent)]
  WindowHandleError(#[from] raw_window_handle::HandleError),
  #[error("the window handle kind is not supported")]
  UnsupportedWindowHandle,
  #[error(transparent)]
  Utf8Error(#[from] std::str::Utf8Error),
  #[cfg(target_os = "android")]
  #[error(transparent)]
  CrossBeamRecvError(#[from] crossbeam_channel::RecvError),
  #[error("not on the main thread")]
  NotMainThread,
  #[error("Custom protocol task is invalid.")]
  CustomProtocolTaskInvalid,
  #[error("Failed to register URL scheme: {0}, could be due to invalid URL scheme or the scheme is already registered.")]
  UrlSchemeRegisterError(String),
  #[error("Duplicate custom protocol '{0}' registered on the WebViewBuilder")]
  DuplicateCustomProtocol(String),
  #[error("Duplicate custom protocol '{0}' registered on the same web context on Linux")]
  ContextDuplicateCustomProtocol(String),
  #[error(transparent)]
  #[cfg(any(target_os = "macos", target_os = "ios"))]
  UrlParse(#[from] url::ParseError),
  #[cfg(any(target_os = "macos", target_os = "ios"))]
  #[error("data store is currently opened")]
  DataStoreInUse,
  #[cfg(target_os = "android")]
  #[error("Activity not found")]
  ActivityNotFound,
}
