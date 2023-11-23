use std::sync::Arc;

use raw_window_handle::HasRawWindowHandle;
use url::Url;

use crate::{Rect, Result, WebContext, WebViewAttributes, RGBA};

use self::embedder::Embedder;

mod embedder;
mod prefs;
mod resources;
mod window;

pub(crate) struct InnerWebView {
  embedder: Embedder,
}

impl InnerWebView {
  pub fn new<W: HasRawWindowHandle>(
    window: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    resources::init(web_context);
    prefs::init();

    // TODO callback attributes
    let embedder = Embedder::new(window.raw_window_handle(), Arc::new(||{}));

    Ok(Self { embedder })
  }

  pub fn new_as_child<W: HasRawWindowHandle>(
    parent: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    // Ok(Self)
    todo!()
  }

  pub fn print(&self) {}

  pub fn url(&self) -> Url {
    Url::parse("").unwrap()
  }

  pub fn eval(
    &self,
    js: &str,
    callback: Option<impl FnOnce(String) + Send + 'static>,
  ) -> Result<()> {
    Ok(())
  }

  fn init(&self, js: &str) -> Result<()> {
    Ok(())
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    true
  }

  pub fn zoom(&self, scale_factor: f64) {}

  pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
    Ok(())
  }

  pub fn load_url(&self, url: &str) {}

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) {}

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    Ok(())
  }

  pub fn set_bounds(&self, bounds: Rect) {
    // self.handle(ServoEvent::ResizeWebView(bounds));
  }

  pub fn set_visible(&self, visible: bool) {}

  pub fn focus(&self) {}
}

pub fn platform_webview_version() -> Result<String> {
  Ok(String::from(""))
}
