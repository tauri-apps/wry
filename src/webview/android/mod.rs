use super::{WebContext, WebViewAttributes};
use crate::{
  application::window::Window,
  http::{Request as HttpRequest, Response as HttpResponse},
  Result,
};
use once_cell::sync::OnceCell;
use std::rc::Rc;
use tao::platform::android::ndk_glue::{
  jni::{objects::GlobalRef, JNIEnv},
  ndk::looper::{FdEvent, ForeignLooper},
};

pub(crate) mod binding;
mod main_pipe;
use main_pipe::{MainPipe, WebViewMessage, MAIN_PIPE};

#[macro_export]
macro_rules! android_binding {
  ($domain:ident, $package:ident, $main: ident) => {
    android_binding!($domain, $package, $main, ::wry)
  };
  ($domain:ident, $package:ident, $main: ident, $wry: path) => {
    use $wry::{
      application::{
        android_binding as tao_android_binding, android_fn, platform::android::ndk_glue::*,
      },
      webview::prelude::*,
    };
    tao_android_binding!($domain, $package, setup, $main);
    android_fn!(
      $domain,
      $package,
      RustWebChromeClient,
      runInitializationScripts
    );
    android_fn!(
      $domain,
      $package,
      RustWebViewClient,
      handleRequest,
      JObject,
      jobject
    );
    android_fn!($domain, $package, Ipc, ipc, JString);
  };
}

pub static IPC: OnceCell<UnsafeIpc> = OnceCell::new();
pub static REQUEST_HANDLER: OnceCell<UnsafeRequestHandler> = OnceCell::new();

pub struct UnsafeIpc(Box<dyn Fn(&Window, String)>, Rc<Window>);
impl UnsafeIpc {
  pub fn new(f: Box<dyn Fn(&Window, String)>, w: Rc<Window>) -> Self {
    Self(f, w)
  }
}
unsafe impl Send for UnsafeIpc {}
unsafe impl Sync for UnsafeIpc {}

pub struct UnsafeRequestHandler(Box<dyn Fn(HttpRequest) -> Option<HttpResponse>>);
impl UnsafeRequestHandler {
  pub fn new(f: Box<dyn Fn(HttpRequest) -> Option<HttpResponse>>) -> Self {
    Self(f)
  }
}
unsafe impl Send for UnsafeRequestHandler {}
unsafe impl Sync for UnsafeRequestHandler {}

pub unsafe fn setup(env: JNIEnv, looper: &ForeignLooper, activity: GlobalRef) {
  let mut main_pipe = MainPipe {
    env,
    activity,
    initialization_scripts: vec![],
    webview: None,
  };

  looper
    .add_fd_with_callback(MAIN_PIPE[0], FdEvent::INPUT, move |_| {
      let size = std::mem::size_of::<bool>();
      let mut wake = false;
      if libc::read(MAIN_PIPE[0], &mut wake as *mut _ as *mut _, size) == size as libc::ssize_t {
        match main_pipe.recv() {
          Ok(_) => true,
          Err(_) => false,
        }
      } else {
        false
      }
    })
    .unwrap();
}

pub struct InnerWebView {
  pub window: Rc<Window>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    attributes: WebViewAttributes,
    _web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let WebViewAttributes {
      url,
      initialization_scripts,
      ipc_handler,
      devtools,
      custom_protocols,
      ..
    } = attributes;

    if let Some(u) = url {
      let mut url_string = String::from(u.as_str());
      let name = u.scheme();
      let schemes = custom_protocols
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
      if schemes.contains(&name) {
        url_string = u
          .as_str()
          .replace(&format!("{}://", name), &format!("https://{}.", name))
      }

      MainPipe::send(WebViewMessage::CreateWebView(
        url_string,
        initialization_scripts,
        devtools,
      ));
    }

    REQUEST_HANDLER.get_or_init(move || {
      UnsafeRequestHandler::new(Box::new(move |mut request| {
        if let Some(custom_protocol) = custom_protocols
          .iter()
          .find(|(name, _)| request.uri().starts_with(&format!("https://{}.", name)))
        {
          *request.uri_mut() = request.uri().replace(
            &format!("https://{}.", custom_protocol.0),
            &format!("{}://", custom_protocol.0),
          );

          if let Ok(response) = (custom_protocol.1)(&request) {
            return Some(response);
          }
        }

        None
      }))
    });

    let w = window.clone();
    if let Some(i) = ipc_handler {
      IPC.get_or_init(move || UnsafeIpc::new(Box::new(i), w));
    }

    Ok(Self { window })
  }

  pub fn print(&self) {}

  pub fn eval(&self, js: &str) -> Result<()> {
    MainPipe::send(WebViewMessage::Eval(js.into()));
    Ok(())
  }

  pub fn focus(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    false
  }

  pub fn zoom(&self, _scale_factor: f64) {}
}

pub fn platform_webview_version() -> Result<String> {
  todo!()
}
