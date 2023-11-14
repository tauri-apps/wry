use raw_window_handle::HasRawWindowHandle;
use url::Url;

use crate::{Result, WebContext, WebViewAttributes, RGBA};

mod embedder;
mod prefs;
mod resources;

pub(crate) struct InnerWebView;

impl InnerWebView {
  pub fn new<W: HasRawWindowHandle>(
    window: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    resources::init(web_context);
    prefs::init();
    Ok(Self)
  }

  pub fn new_as_child<W: HasRawWindowHandle>(
    parent: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    Ok(Self)
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

  pub fn set_position(&self, position: (i32, i32)) {}

  pub fn set_size(&self, size: (u32, u32)) {}

  pub fn set_visible(&self, visible: bool) {}

  pub fn focus(&self) {}
}

pub fn platform_webview_version() -> Result<String> {
  Ok(String::from(""))
}
