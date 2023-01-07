// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use super::{WebContext, WebViewAttributes, RGBA};
use crate::{application::window::Window, Result};
use crossbeam_channel::*;
use html5ever::{interface::QualName, namespace_url, ns, tendril::TendrilSink, LocalName};
use http::{
  header::{HeaderValue, CONTENT_SECURITY_POLICY, CONTENT_TYPE},
  Request, Response,
};
use kuchiki::NodeRef;
use once_cell::sync::OnceCell;
use sha2::{Digest, Sha256};
use std::{borrow::Cow, rc::Rc};
use tao::platform::android::ndk_glue::{
  jni::{
    errors::Error as JniError,
    objects::{GlobalRef, JClass, JObject},
    JNIEnv,
  },
  ndk::looper::{FdEvent, ForeignLooper},
  JMap, PACKAGE,
};
use url::Url;

pub(crate) mod binding;
mod main_pipe;
use main_pipe::{CreateWebViewAttributes, MainPipe, WebViewMessage, MAIN_PIPE};

#[macro_export]
macro_rules! android_binding {
  ($domain:ident, $package:ident, $main: ident) => {
    android_binding!($domain, $package, $main, ::wry)
  };
  ($domain:ident, $package:ident, $main: ident, $wry: path) => {
    use $wry::{
      application::{
        android_binding as tao_android_binding, android_fn, generate_package_name,
        platform::android::ndk_glue::*,
      },
      webview::prelude::*,
    };
    tao_android_binding!($domain, $package, setup, $main);
    android_fn!(
      $domain,
      $package,
      RustWebViewClient,
      handleRequest,
      [JObject],
      jobject
    );
    android_fn!($domain, $package, Ipc, ipc, [JString]);
    android_fn!(
      $domain,
      $package,
      RustWebChromeClient,
      handleReceivedTitle,
      [JObject, JString],
    );
  };
}

pub static IPC: OnceCell<UnsafeIpc> = OnceCell::new();
pub static REQUEST_HANDLER: OnceCell<UnsafeRequestHandler> = OnceCell::new();
pub static TITLE_CHANGE_HANDLER: OnceCell<UnsafeTitleHandler> = OnceCell::new();

pub struct UnsafeIpc(Box<dyn Fn(&Window, String)>, Rc<Window>);
impl UnsafeIpc {
  pub fn new(f: Box<dyn Fn(&Window, String)>, w: Rc<Window>) -> Self {
    Self(f, w)
  }
}
unsafe impl Send for UnsafeIpc {}
unsafe impl Sync for UnsafeIpc {}

pub struct UnsafeRequestHandler(
  Box<dyn Fn(Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>>>,
);
impl UnsafeRequestHandler {
  pub fn new(f: Box<dyn Fn(Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>>>) -> Self {
    Self(f)
  }
}
unsafe impl Send for UnsafeRequestHandler {}
unsafe impl Sync for UnsafeRequestHandler {}

pub struct UnsafeTitleHandler(Box<dyn Fn(&Window, String)>, Rc<Window>);
impl UnsafeTitleHandler {
  pub fn new(f: Box<dyn Fn(&Window, String)>, w: Rc<Window>) -> Self {
    Self(f, w)
  }
}
unsafe impl Send for UnsafeTitleHandler {}
unsafe impl Sync for UnsafeTitleHandler {}

pub unsafe fn setup(env: JNIEnv, looper: &ForeignLooper, activity: GlobalRef) {
  // we must create the WebChromeClient here because it calls `registerForActivityResult`,
  // which gives an `LifecycleOwners must call register before they are STARTED.` error when called outside the onCreate hook
  let rust_webchrome_client_class = find_my_class(
    env,
    activity.as_obj(),
    format!("{}/RustWebChromeClient", PACKAGE.get().unwrap()),
  )
  .unwrap();
  let webchrome_client = env
    .new_object(
      rust_webchrome_client_class,
      "(Landroidx/appcompat/app/AppCompatActivity;)V",
      &[activity.as_obj().into()],
    )
    .unwrap();

  let mut main_pipe = MainPipe {
    env,
    activity,
    webview: None,
    webchrome_client: env.new_global_ref(webchrome_client).unwrap(),
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

pub(crate) struct InnerWebView {
  #[allow(unused)]
  pub window: Rc<Window>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    attributes: WebViewAttributes,
    _pl_attrs: super::PlatformSpecificWebViewAttributes,
    _web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let WebViewAttributes {
      url,
      initialization_scripts,
      ipc_handler,
      devtools,
      custom_protocols,
      background_color,
      transparent,
      headers,
      ..
    } = attributes;

    if let Some(u) = url {
      let mut url_string = String::from(u.as_str());
      let name = u.scheme();
      let is_custom_protocol = custom_protocols.iter().any(|(n, _)| n == name);
      if is_custom_protocol {
        url_string = u
          .as_str()
          .replace(&format!("{}://", name), &format!("https://{}.", name))
      }

      MainPipe::send(WebViewMessage::CreateWebView(CreateWebViewAttributes {
        url: url_string,
        devtools,
        background_color,
        transparent,
        headers,
      }));
    }

    REQUEST_HANDLER.get_or_init(move || {
      UnsafeRequestHandler::new(Box::new(move |mut request| {
        if let Some(custom_protocol) = custom_protocols.iter().find(|(name, _)| {
          request
            .uri()
            .to_string()
            .starts_with(&format!("https://{}.", name))
        }) {
          *request.uri_mut() = request
            .uri()
            .to_string()
            .replace(
              &format!("https://{}.", custom_protocol.0),
              &format!("{}://", custom_protocol.0),
            )
            .parse()
            .unwrap();

          if let Ok(mut response) = (custom_protocol.1)(&request) {
            if response.headers().get(CONTENT_TYPE) == Some(&HeaderValue::from_static("text/html"))
            {
              if !initialization_scripts.is_empty() {
                let mut document =
                  kuchiki::parse_html().one(String::from_utf8_lossy(response.body()).into_owned());
                let csp = response.headers_mut().get_mut(CONTENT_SECURITY_POLICY);
                let mut hashes = Vec::new();
                with_html_head(&mut document, |head| {
                  // iterate in reverse order since we are prepending each script to the head tag
                  for script in initialization_scripts.iter().rev() {
                    let script_el =
                      NodeRef::new_element(QualName::new(None, ns!(html), "script".into()), None);
                    script_el.append(NodeRef::new_text(script));
                    head.prepend(script_el);
                    if csp.is_some() {
                      hashes.push(hash_script(script));
                    }
                  }
                });

                if let Some(csp) = csp {
                  let csp_string = csp.to_str().unwrap().to_string();
                  let csp_string = if csp_string.contains("script-src") {
                    csp_string.replace("script-src", &format!("script-src {}", hashes.join(" ")))
                  } else {
                    format!("{} script-src {}", csp_string, hashes.join(" "))
                  };
                  *csp = HeaderValue::from_str(&csp_string).unwrap();
                }

                *response.body_mut() = document.to_string().into_bytes().into();
              }
            }
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

    let w = window.clone();
    if let Some(i) = attributes.document_title_changed_handler {
      TITLE_CHANGE_HANDLER.get_or_init(move || UnsafeTitleHandler::new(i, w));
    }

    Ok(Self { window })
  }

  pub fn print(&self) {}

  pub fn url(&self) -> Url {
    let (tx, rx) = bounded(1);
    MainPipe::send(WebViewMessage::GetUrl(tx));
    let uri = rx.recv().unwrap();
    Url::parse(uri.as_str()).unwrap()
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    MainPipe::send(WebViewMessage::Eval(js.into()));
    Ok(())
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    false
  }

  pub fn zoom(&self, _scale_factor: f64) {}

  pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
    MainPipe::send(WebViewMessage::SetBackgroundColor(background_color));
    Ok(())
  }

  pub fn load_url(&self, url: &str) {
    MainPipe::send(WebViewMessage::LoadUrl(url.to_string(), None));
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) {
    MainPipe::send(WebViewMessage::LoadUrl(url.to_string(), Some(headers)));
  }
}

#[derive(Clone, Copy)]
pub struct JniHandle;

impl JniHandle {
  /// Execute jni code on the thread of the webview.
  /// Provided function will be provided with the jni evironment, Android activity and WebView
  pub fn exec<F>(&self, func: F)
  where
    F: FnOnce(JNIEnv, JObject, JObject) + Send + 'static,
  {
    MainPipe::send(WebViewMessage::Jni(Box::new(func)));
  }
}

pub fn platform_webview_version() -> Result<String> {
  let (tx, rx) = bounded(1);
  MainPipe::send(WebViewMessage::GetWebViewVersion(tx));
  rx.recv().unwrap()
}

fn with_html_head<F: FnOnce(&NodeRef)>(document: &mut NodeRef, f: F) {
  if let Ok(ref node) = document.select_first("head") {
    f(node.as_node())
  } else {
    let node = NodeRef::new_element(
      QualName::new(None, ns!(html), LocalName::from("head")),
      None,
    );
    f(&node);
    document.prepend(node)
  }
}

fn hash_script(script: &str) -> String {
  let mut hasher = Sha256::new();
  hasher.update(script);
  let hash = hasher.finalize();
  format!("'sha256-{}'", base64::encode(hash))
}

fn find_my_class<'a>(
  env: JNIEnv<'a>,
  activity: JObject<'a>,
  name: String,
) -> std::result::Result<JClass<'a>, JniError> {
  let class_name = env.new_string(name.replace('/', "."))?;
  let my_class = env
    .call_method(
      activity,
      "getAppClass",
      "(Ljava/lang/String;)Ljava/lang/Class;",
      &[class_name.into()],
    )?
    .l()?;
  Ok(my_class.into())
}

fn create_headers_map<'a, 'b>(
  env: &'a JNIEnv,
  headers: &http::HeaderMap,
) -> std::result::Result<JMap<'a, 'b>, JniError> {
  let obj = env.new_object("java/util/HashMap", "()V", &[])?;
  let headers_map = JMap::from_env(&env, obj)?;
  for (name, value) in headers.iter() {
    let key = env.new_string(name)?;
    let value = env.new_string(value.to_str().unwrap_or_default())?;
    headers_map.put(key.into(), value.into())?;
  }
  Ok(headers_map)
}
