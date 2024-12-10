// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use super::{PageLoadEvent, WebViewAttributes, RGBA};
use crate::{RequestAsyncResponder, Result};
use base64::{engine::general_purpose, Engine};
use crossbeam_channel::*;
use html5ever::{interface::QualName, namespace_url, ns, tendril::TendrilSink, LocalName};
use http::{
  header::{HeaderValue, CONTENT_SECURITY_POLICY, CONTENT_TYPE},
  Request, Response as HttpResponse,
};
use jni::{
  errors::Result as JniResult,
  objects::{GlobalRef, JClass, JObject},
  JNIEnv,
};
use kuchiki::NodeRef;
use ndk::looper::{FdEvent, ThreadLooper};
use once_cell::sync::OnceCell;
use raw_window_handle::HasWindowHandle;
use sha2::{Digest, Sha256};
use std::{
  borrow::Cow,
  cell::RefCell,
  collections::HashMap,
  os::fd::{AsFd as _, AsRawFd as _},
  sync::{mpsc::channel, Mutex},
  time::Duration,
};

pub(crate) mod binding;
mod main_pipe;
use main_pipe::{CreateWebViewAttributes, MainPipe, MainPipeState, WebViewMessage, MAIN_PIPE};

use crate::util::Counter;

static COUNTER: Counter = Counter::new();
const MAIN_PIPE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Context<'a, 'b> {
  pub env: &'a mut JNIEnv<'b>,
  pub activity: &'a JObject<'b>,
  pub webview: &'a JObject<'b>,
}

pub(crate) struct StaticCell<T>(RefCell<T>);

unsafe impl<T> Send for StaticCell<T> {}
unsafe impl<T> Sync for StaticCell<T> {}

impl<T> std::ops::Deref for StaticCell<T> {
  type Target = RefCell<T>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

macro_rules! define_static_handlers {
  ($($var:ident = $type_name:ident { $($fields:ident:$types:ty),+ $(,)? });+ $(;)?) => {
    $(pub static $var: StaticCell<Option<$type_name>> = StaticCell(RefCell::new(None));
    pub struct $type_name {
      $($fields: $types,)*
    }
    impl $type_name {
      pub fn new($($fields: $types,)*) -> Self {
        Self {
          $($fields,)*
        }
      }
    }
    unsafe impl Send for $type_name {}
    unsafe impl Sync for $type_name {})*
  };
}

define_static_handlers! {
  IPC =  UnsafeIpc { handler: Box<dyn Fn(Request<String>)> };
  REQUEST_HANDLER = UnsafeRequestHandler { handler:  Box<dyn Fn(&str, Request<Vec<u8>>, bool) -> Option<HttpResponse<Cow<'static, [u8]>>>> };
  TITLE_CHANGE_HANDLER = UnsafeTitleHandler { handler: Box<dyn Fn(String)> };
  URL_LOADING_OVERRIDE = UnsafeUrlLoadingOverride { handler: Box<dyn Fn(String) -> bool> };
  ON_LOAD_HANDLER = UnsafeOnPageLoadHandler { handler: Box<dyn Fn(PageLoadEvent, String)> };
}

pub static WITH_ASSET_LOADER: StaticCell<Option<bool>> = StaticCell(RefCell::new(None));
pub static ASSET_LOADER_DOMAIN: StaticCell<Option<String>> = StaticCell(RefCell::new(None));

pub(crate) static PACKAGE: OnceCell<String> = OnceCell::new();

type EvalCallback = Box<dyn Fn(String) + Send + 'static>;

pub static EVAL_ID_GENERATOR: Counter = Counter::new();
pub static EVAL_CALLBACKS: OnceCell<Mutex<HashMap<i32, EvalCallback>>> = OnceCell::new();

/// Sets up the necessary logic for wry to be able to create the webviews later.
///
/// This function must be run on the thread where the [`JNIEnv`] is registered and the looper is local,
/// hence the requirement for a [`ThreadLooper`].
pub unsafe fn android_setup(
  package: &str,
  mut env: JNIEnv,
  looper: &ThreadLooper,
  activity: GlobalRef,
) {
  PACKAGE.get_or_init(move || package.to_string());

  // we must create the WebChromeClient here because it calls `registerForActivityResult`,
  // which gives an `LifecycleOwners must call register before they are STARTED.` error when called outside the onCreate hook
  let rust_webchrome_client_class = find_class(
    &mut env,
    activity.as_obj(),
    format!("{}/RustWebChromeClient", PACKAGE.get().unwrap()),
  )
  .unwrap();
  let webchrome_client = env
    .new_object(
      &rust_webchrome_client_class,
      &format!("(L{}/WryActivity;)V", PACKAGE.get().unwrap()),
      &[activity.as_obj().into()],
    )
    .unwrap();

  let webchrome_client = env.new_global_ref(webchrome_client).unwrap();
  let mut main_pipe = MainPipe {
    env,
    activity,
    webview: None,
    webchrome_client,
  };

  looper
    .add_fd_with_callback(MAIN_PIPE[0].as_fd(), FdEvent::INPUT, move |fd, _event| {
      let size = std::mem::size_of::<bool>();
      let mut wake = false;
      if libc::read(fd.as_raw_fd(), &mut wake as *mut _ as *mut _, size) == size as libc::ssize_t {
        let res = main_pipe.recv();
        // unregister itself on errors or destroy event
        matches!(res, Ok(MainPipeState::Alive))
      } else {
        // unregister itself
        false
      }
    })
    .unwrap();
}

pub(crate) struct InnerWebView {
  id: String,
}

impl InnerWebView {
  pub fn new_as_child(
    _window: &impl HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
    Self::new(_window, attributes, pl_attrs)
  }

  pub fn new(
    _window: &impl HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
    let WebViewAttributes {
      url,
      html,
      initialization_scripts,
      ipc_handler,
      #[cfg(any(debug_assertions, feature = "devtools"))]
      devtools,
      custom_protocols,
      background_color,
      transparent,
      headers,
      autoplay,
      user_agent,
      ..
    } = attributes;

    let super::PlatformSpecificWebViewAttributes {
      on_webview_created,
      with_asset_loader,
      asset_loader_domain,
      https_scheme,
    } = pl_attrs;

    let scheme = if https_scheme { "https" } else { "http" };

    let url = if let Some(mut url) = url {
      if let Some(pos) = url.find("://") {
        let name = &url[..pos];
        let is_custom_protocol = custom_protocols.iter().any(|(n, _)| n == name);
        if is_custom_protocol {
          url = url.replace(&format!("{name}://"), &format!("{scheme}://{name}."))
        }
      }

      Some(url)
    } else {
      None
    };

    let id = attributes
      .id
      .map(|id| id.to_string())
      .unwrap_or_else(|| COUNTER.next().to_string());

    MainPipe::send(WebViewMessage::CreateWebView(CreateWebViewAttributes {
      id: id.clone(),
      url,
      html,
      #[cfg(any(debug_assertions, feature = "devtools"))]
      devtools,
      background_color,
      transparent,
      headers,
      on_webview_created,
      autoplay,
      user_agent,
      initialization_scripts: initialization_scripts.clone(),
    }));

    WITH_ASSET_LOADER.replace(Some(with_asset_loader));
    if let Some(domain) = asset_loader_domain {
      ASSET_LOADER_DOMAIN.replace(Some(domain));
    }

    REQUEST_HANDLER.replace(Some(
      UnsafeRequestHandler::new(Box::new(
        move |webview_id: &str, mut request, is_document_start_script_enabled| {
          let uri = request.uri().to_string();
          if let Some((custom_protocol_uri, custom_protocol_closure)) = custom_protocols.iter().find(|(name, _)| {
            uri.starts_with(&format!("{scheme}://{}.", name))
          }) {
            let uri_res = uri
              .replace(
                &format!("{scheme}://{}.", custom_protocol_uri),
                &format!("{}://", custom_protocol_uri),
              )
              .parse();

            if let Ok(uri) = uri_res {
              *request.uri_mut() = uri;
            }

            let (tx, rx) = channel();
            let initialization_scripts = initialization_scripts.clone();
            let responder: Box<dyn FnOnce(HttpResponse<Cow<'static, [u8]>>)> =
              Box::new(move |mut response| {
                if !is_document_start_script_enabled {
                  #[cfg(feature = "tracing")]
                  tracing::info!("`addDocumentStartJavaScript` is not supported; injecting initialization scripts via custom protocol handler");
                  let should_inject_scripts = response
                    .headers()
                    .get(CONTENT_TYPE)
                    // Content-Type must begin with the media type, but is case-insensitive.
                    // It may also be followed by any number of semicolon-delimited key value pairs.
                    // We don't care about these here.
                    // source: https://httpwg.org/specs/rfc9110.html#rfc.section.8.3.1
                    .and_then(|content_type| content_type.to_str().ok())
                    .map(|content_type_str| {
                      content_type_str.to_lowercase().starts_with("text/html")
                    })
                    .unwrap_or_default();

                  if should_inject_scripts && !initialization_scripts.is_empty() {
                    let mut document = kuchiki::parse_html()
                      .one(String::from_utf8_lossy(response.body()).into_owned());
                    let csp = response.headers_mut().get_mut(CONTENT_SECURITY_POLICY);
                    let mut hashes = Vec::new();
                    with_html_head(&mut document, |head| {
                      // iterate in reverse order since we are prepending each script to the head tag
                      for (script, _) in initialization_scripts.iter().rev() {
                        let script_el = NodeRef::new_element(
                          QualName::new(None, ns!(html), "script".into()),
                          None,
                        );
                        script_el.append(NodeRef::new_text(script.as_str()));
                        head.prepend(script_el);
                        if csp.is_some() {
                          hashes.push(hash_script(script.as_str()));
                        }
                      }
                    });

                    if let Some(csp) = csp {
                      let csp_string = csp.to_str().unwrap().to_string();
                      let csp_string = if csp_string.contains("script-src") {
                        csp_string
                          .replace("script-src", &format!("script-src {}", hashes.join(" ")))
                      } else {
                        format!("{} script-src {}", csp_string, hashes.join(" "))
                      };
                      *csp = HeaderValue::from_str(&csp_string).unwrap();
                    }

                    *response.body_mut() = document.to_string().into_bytes().into();
                  }
                }

                tx.send(response).unwrap();
              });

            (custom_protocol_closure)(webview_id, request, RequestAsyncResponder { responder });
            return Some(rx.recv_timeout(MAIN_PIPE_TIMEOUT).unwrap());
          }
          None
        },
      ))
    ));

    if let Some(i) = ipc_handler {
      IPC.replace(Some(UnsafeIpc::new(Box::new(i))));
    }

    if let Some(i) = attributes.document_title_changed_handler {
      TITLE_CHANGE_HANDLER.replace(Some(UnsafeTitleHandler::new(i)));
    }

    if let Some(i) = attributes.navigation_handler {
      URL_LOADING_OVERRIDE.replace(Some(UnsafeUrlLoadingOverride::new(i)));
    }

    if let Some(h) = attributes.on_page_load_handler {
      ON_LOAD_HANDLER.replace(Some(UnsafeOnPageLoadHandler::new(h)));
    }

    Ok(Self { id })
  }

  pub fn print(&self) -> crate::Result<()> {
    Ok(())
  }

  pub fn id(&self) -> crate::WebViewId {
    &self.id
  }

  pub fn url(&self) -> crate::Result<String> {
    let (tx, rx) = bounded(1);
    MainPipe::send(WebViewMessage::GetUrl(tx));
    rx.recv_timeout(MAIN_PIPE_TIMEOUT).map_err(Into::into)
  }

  pub fn eval(&self, js: &str, callback: Option<impl Fn(String) + Send + 'static>) -> Result<()> {
    MainPipe::send(WebViewMessage::Eval(
      js.into(),
      callback.map(|c| Box::new(c) as Box<dyn Fn(String) + Send + 'static>),
    ));
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

  pub fn zoom(&self, _scale_factor: f64) -> Result<()> {
    Ok(())
  }

  pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
    MainPipe::send(WebViewMessage::SetBackgroundColor(background_color));
    Ok(())
  }

  pub fn load_url(&self, url: &str) -> Result<()> {
    MainPipe::send(WebViewMessage::LoadUrl(url.to_string(), None));
    Ok(())
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) -> Result<()> {
    MainPipe::send(WebViewMessage::LoadUrl(url.to_string(), Some(headers)));
    Ok(())
  }

  pub fn load_html(&self, html: &str) -> Result<()> {
    MainPipe::send(WebViewMessage::LoadHtml(html.to_string()));
    Ok(())
  }

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    MainPipe::send(WebViewMessage::ClearAllBrowsingData);
    Ok(())
  }

  pub fn cookies_for_url(&self, url: &str) -> Result<Vec<cookie::Cookie<'static>>> {
    let (tx, rx) = bounded(1);
    MainPipe::send(WebViewMessage::GetCookies(tx, url.to_string()));
    rx.recv_timeout(MAIN_PIPE_TIMEOUT).map_err(Into::into)
  }

  pub fn cookies(&self) -> Result<Vec<cookie::Cookie<'static>>> {
    Ok(Vec::new())
  }

  pub fn bounds(&self) -> Result<crate::Rect> {
    Ok(crate::Rect::default())
  }

  pub fn set_bounds(&self, _bounds: crate::Rect) -> Result<()> {
    // Unsupported
    Ok(())
  }

  pub fn set_visible(&self, _visible: bool) -> Result<()> {
    // Unsupported
    Ok(())
  }

  pub fn focus(&self) -> Result<()> {
    // Unsupported
    Ok(())
  }

  pub fn focus_parent(&self) -> Result<()> {
    // Unsupported
    Ok(())
  }
}

#[derive(Clone, Copy)]
pub struct JniHandle;

impl JniHandle {
  /// Execute jni code on the thread of the webview.
  /// Provided function will be provided with the jni evironment, Android activity and WebView
  pub fn exec<F>(&self, func: F)
  where
    F: FnOnce(&mut JNIEnv, &JObject, &JObject) + Send + 'static,
  {
    MainPipe::send(WebViewMessage::Jni(Box::new(func)));
  }
}

pub fn platform_webview_version() -> Result<String> {
  let (tx, rx) = bounded(1);
  MainPipe::send(WebViewMessage::GetWebViewVersion(tx));
  rx.recv_timeout(MAIN_PIPE_TIMEOUT).unwrap()
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
  format!("'sha256-{}'", general_purpose::STANDARD.encode(hash))
}

/// Finds a class in the project scope.
pub fn find_class<'a>(
  env: &mut JNIEnv<'a>,
  activity: &JObject<'_>,
  name: String,
) -> JniResult<JClass<'a>> {
  let class_name = env.new_string(name.replace('/', "."))?;
  let my_class = env
    .call_method(
      activity,
      "getAppClass",
      "(Ljava/lang/String;)Ljava/lang/Class;",
      &[(&class_name).into()],
    )?
    .l()?;
  Ok(my_class.into())
}

/// Dispatch a closure to run on the Android context.
///
/// The closure takes the JNI env, the Android activity instance and the possibly null webview.
pub fn dispatch<F>(func: F)
where
  F: FnOnce(&mut JNIEnv, &JObject, &JObject) + Send + 'static,
{
  MainPipe::send(WebViewMessage::Jni(Box::new(func)));
}
