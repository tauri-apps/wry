use std::rc::Rc;

use crate::{application::window::Window, Result};

use super::{WebContext, WebViewAttributes};

pub struct InnerWebView {
  pub window: Rc<Window>,
  pub attributes: WebViewAttributes,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    attributes: WebViewAttributes,
    _web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    Ok(Self { window, attributes })
  }

  pub fn print(&self) {
    todo!()
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    todo!()
  }

  pub fn focus(&self) {
    todo!()
  }

  pub fn devtool(&self) {
    todo!()
  }
}

pub fn platform_webview_version() -> Result<String> {
  todo!()
}
