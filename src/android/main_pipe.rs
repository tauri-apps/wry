// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{Error, RGBA};
use crossbeam_channel::*;
use jni::{
  errors::Result as JniResult,
  objects::{GlobalRef, JMap, JObject, JString},
  JNIEnv,
};
use once_cell::sync::Lazy;
use std::os::unix::prelude::*;

use super::{find_class, EvalCallback, EVAL_CALLBACKS, EVAL_ID_GENERATOR, PACKAGE};

static CHANNEL: Lazy<(Sender<WebViewMessage>, Receiver<WebViewMessage>)> = Lazy::new(|| bounded(8));
pub static MAIN_PIPE: Lazy<[OwnedFd; 2]> = Lazy::new(|| {
  let mut pipe: [RawFd; 2] = Default::default();
  unsafe { libc::pipe(pipe.as_mut_ptr()) };
  unsafe { pipe.map(|fd| OwnedFd::from_raw_fd(fd)) }
});

pub enum MainPipeState {
  Alive,
  Destroyed,
}

pub struct MainPipe<'a> {
  pub env: JNIEnv<'a>,
  pub activity: GlobalRef,
  pub webview: Option<GlobalRef>,
  pub webchrome_client: GlobalRef,
}

impl<'a> MainPipe<'a> {
  pub(crate) fn send(message: WebViewMessage) {
    let size = std::mem::size_of::<bool>();
    if let Ok(()) = CHANNEL.0.send(message) {
      unsafe {
        libc::write(
          MAIN_PIPE[1].as_raw_fd(),
          &true as *const _ as *const _,
          size,
        )
      };
    }
  }

  pub fn recv(&mut self) -> JniResult<MainPipeState> {
    let activity = self.activity.as_obj();
    if let Ok(message) = CHANNEL.1.recv() {
      match message {
        WebViewMessage::CreateWebView(attrs) => {
          let CreateWebViewAttributes {
            url,
            html,
            #[cfg(any(debug_assertions, feature = "devtools"))]
            devtools,
            transparent,
            background_color,
            headers,
            on_webview_created,
            autoplay,
            user_agent,
            initialization_scripts,
            id,
            ..
          } = attrs;

          let string_class = self.env.find_class("java/lang/String")?;
          let initialization_scripts_array = self.env.new_object_array(
            initialization_scripts.len() as i32,
            string_class,
            self.env.new_string("")?,
          )?;
          for (i, (script, _)) in initialization_scripts.into_iter().enumerate() {
            self.env.set_object_array_element(
              &initialization_scripts_array,
              i as i32,
              self.env.new_string(script)?,
            )?;
          }

          let id = self.env.new_string(id)?;

          // Create webview
          let rust_webview_class = find_class(
            &mut self.env,
            activity,
            format!("{}/RustWebView", PACKAGE.get().unwrap()),
          )?;
          let webview = self.env.new_object(
            &rust_webview_class,
            "(Landroid/content/Context;[Ljava/lang/String;Ljava/lang/String;)V",
            &[
              activity.into(),
              (&initialization_scripts_array).into(),
              (&id).into(),
            ],
          )?;

          // set media autoplay
          self
            .env
            .call_method(&webview, "setAutoPlay", "(Z)V", &[autoplay.into()])?;

          // set user-agent
          if let Some(user_agent) = user_agent {
            let user_agent = self.env.new_string(user_agent)?;
            self.env.call_method(
              &webview,
              "setUserAgent",
              "(Ljava/lang/String;)V",
              &[(&user_agent).into()],
            )?;
          }

          self.env.call_method(
            activity,
            "setWebView",
            format!("(L{}/RustWebView;)V", PACKAGE.get().unwrap()),
            &[(&webview).into()],
          )?;

          // Navigation
          if let Some(u) = url {
            if let Ok(url) = self.env.new_string(u) {
              load_url(&mut self.env, &webview, &url, headers, true)?;
            }
          } else if let Some(h) = html {
            if let Ok(html) = self.env.new_string(h) {
              load_html(&mut self.env, &webview, &html)?;
            }
          }

          // Enable devtools
          #[cfg(any(debug_assertions, feature = "devtools"))]
          self.env.call_static_method(
            &rust_webview_class,
            "setWebContentsDebuggingEnabled",
            "(Z)V",
            &[devtools.into()],
          )?;

          if transparent {
            set_background_color(&mut self.env, &webview, (0, 0, 0, 0))?;
          } else if let Some(color) = background_color {
            set_background_color(&mut self.env, &webview, color)?;
          }

          // Create and set webview client
          let client_class_name = format!("{}/RustWebViewClient", PACKAGE.get().unwrap());
          let rust_webview_client_class =
            find_class(&mut self.env, activity, client_class_name.clone())?;
          let webview_client = self.env.new_object(
            &rust_webview_client_class,
            "(Landroid/content/Context;)V",
            &[activity.into()],
          )?;
          self.env.call_method(
            &webview,
            "setWebViewClient",
            "(Landroid/webkit/WebViewClient;)V",
            &[(&webview_client).into()],
          )?;

          // set webchrome client
          self.env.call_method(
            &webview,
            "setWebChromeClient",
            "(Landroid/webkit/WebChromeClient;)V",
            &[self.webchrome_client.as_obj().into()],
          )?;

          // Add javascript interface (IPC)
          let ipc_class = find_class(
            &mut self.env,
            activity,
            format!("{}/Ipc", PACKAGE.get().unwrap()),
          )?;
          let ipc = self.env.new_object(
            ipc_class,
            format!("(L{client_class_name};)V"),
            &[(&webview_client).into()],
          )?;
          let ipc_str = self.env.new_string("ipc")?;
          self.env.call_method(
            &webview,
            "addJavascriptInterface",
            "(Ljava/lang/Object;Ljava/lang/String;)V",
            &[(&ipc).into(), (&ipc_str).into()],
          )?;

          // Set content view
          self.env.call_method(
            activity,
            "setContentView",
            "(Landroid/view/View;)V",
            &[(&webview).into()],
          )?;

          if let Some(on_webview_created) = on_webview_created {
            if let Err(e) = on_webview_created(super::Context {
              env: &mut self.env,
              activity,
              webview: &webview,
            }) {
              #[cfg(feature = "tracing")]
              tracing::warn!("failed to run webview created hook: {e}");
            }
          }

          let webview = self.env.new_global_ref(webview)?;

          self.webview = Some(webview);
        }
        WebViewMessage::Eval(script, callback) => {
          if let Some(webview) = &self.webview {
            let id = EVAL_ID_GENERATOR.next() as i32;

            #[cfg(feature = "tracing")]
            let span = std::sync::Mutex::new(Some(SendEnteredSpan(
              tracing::debug_span!("wry::eval").entered(),
            )));

            EVAL_CALLBACKS
              .get_or_init(Default::default)
              .lock()
              .unwrap()
              .insert(
                id,
                Box::new(move |result| {
                  #[cfg(feature = "tracing")]
                  span.lock().unwrap().take();

                  if let Some(callback) = &callback {
                    callback(result);
                  }
                }),
              );

            let s = self.env.new_string(script)?;
            self.env.call_method(
              webview.as_obj(),
              "evalScript",
              "(ILjava/lang/String;)V",
              &[id.into(), (&s).into()],
            )?;
          }
        }
        WebViewMessage::SetBackgroundColor(background_color) => {
          if let Some(webview) = &self.webview {
            set_background_color(&mut self.env, webview.as_obj(), background_color)?;
          }
        }
        WebViewMessage::GetWebViewVersion(tx) => {
          match self
            .env
            .call_method(activity, "getVersion", "()Ljava/lang/String;", &[])
            .and_then(|v| v.l())
            .and_then(|s| {
              let s = JString::from(s);
              self
                .env
                .get_string(&s)
                .map(|v| v.to_string_lossy().to_string())
            }) {
            Ok(version) => {
              tx.send(Ok(version)).unwrap();
            }
            Err(e) => tx.send(Err(e.into())).unwrap(),
          }
        }
        WebViewMessage::GetUrl(tx) => {
          if let Some(webview) = &self.webview {
            let url = self
              .env
              .call_method(webview.as_obj(), "getUrl", "()Ljava/lang/String;", &[])
              .and_then(|v| v.l())
              .and_then(|s| {
                let s = JString::from(s);
                self
                  .env
                  .get_string(&s)
                  .map(|v| v.to_string_lossy().to_string())
              })
              .unwrap_or_default();

            tx.send(url).unwrap()
          }
        }
        WebViewMessage::Jni(f) => {
          if let Some(w) = &self.webview {
            f(&mut self.env, activity, w.as_obj());
          } else {
            f(&mut self.env, activity, &JObject::null());
          }
        }
        WebViewMessage::LoadUrl(url, headers) => {
          if let Some(webview) = &self.webview {
            let url = self.env.new_string(url)?;
            load_url(&mut self.env, webview.as_obj(), &url, headers, false)?;
          }
        }
        WebViewMessage::ClearAllBrowsingData => {
          if let Some(webview) = &self.webview {
            self
              .env
              .call_method(webview, "clearAllBrowsingData", "()V", &[])?;
          }
        }
        WebViewMessage::LoadHtml(html) => {
          if let Some(webview) = &self.webview {
            let html = self.env.new_string(html)?;
            load_html(&mut self.env, webview.as_obj(), &html)?;
          }
        }
        WebViewMessage::GetCookies(tx, url) => {
          if let Some(webview) = &self.webview {
            let url = self.env.new_string(url)?;
            let cookies = self
              .env
              .call_method(
                webview,
                "getCookies",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[(&url).into()],
              )
              .and_then(|v| v.l())
              .and_then(|s| {
                let s = JString::from(s);
                self
                  .env
                  .get_string(&s)
                  .map(|v| v.to_string_lossy().to_string())
              })
              .unwrap_or_default();

            tx.send(
              cookies
                .split("; ")
                .flat_map(|c| cookie::Cookie::parse(c.to_string()))
                .collect(),
            )
            .unwrap();
          }
        }
        WebViewMessage::OnDestroy => {
          return Ok(MainPipeState::Destroyed);
        }
      }
    }
    Ok(MainPipeState::Alive)
  }
}

fn load_url<'a>(
  env: &mut JNIEnv<'a>,
  webview: &JObject<'a>,
  url: &JString<'a>,
  headers: Option<http::HeaderMap>,
  main_thread: bool,
) -> JniResult<()> {
  let function = if main_thread {
    "loadUrlMainThread"
  } else {
    "loadUrl"
  };
  if let Some(headers) = headers {
    let obj = env.new_object("java/util/HashMap", "()V", &[])?;
    let headers_map = {
      let headers_map = JMap::from_env(env, &obj)?;
      for (name, value) in headers.iter() {
        let key = env.new_string(name)?;
        let value = env.new_string(value.to_str().unwrap_or_default())?;
        headers_map.put(env, &key, &value)?;
      }
      headers_map
    };
    env.call_method(
      webview,
      function,
      "(Ljava/lang/String;Ljava/util/Map;)V",
      &[url.into(), (&headers_map).into()],
    )?;
  } else {
    env.call_method(webview, function, "(Ljava/lang/String;)V", &[url.into()])?;
  }
  Ok(())
}

fn load_html<'a>(env: &mut JNIEnv<'a>, webview: &JObject<'a>, html: &JString<'a>) -> JniResult<()> {
  env.call_method(
    webview,
    "loadHTMLMainThread",
    "(Ljava/lang/String;)V",
    &[html.into()],
  )?;
  Ok(())
}

fn set_background_color<'a>(
  env: &mut JNIEnv<'a>,
  webview: &JObject<'a>,
  (r, g, b, a): RGBA,
) -> JniResult<()> {
  let color = (a as i32) << 24 | (r as i32) << 16 | (g as i32) << 8 | (b as i32);
  env.call_method(webview, "setBackgroundColor", "(I)V", &[color.into()])?;
  Ok(())
}

pub(crate) enum WebViewMessage {
  CreateWebView(CreateWebViewAttributes),
  Eval(String, Option<EvalCallback>),
  SetBackgroundColor(RGBA),
  GetWebViewVersion(Sender<Result<String, Error>>),
  GetUrl(Sender<String>),
  GetCookies(Sender<Vec<cookie::Cookie<'static>>>, String),
  Jni(Box<dyn FnOnce(&mut JNIEnv, &JObject, &JObject) + Send>),
  LoadUrl(String, Option<http::HeaderMap>),
  LoadHtml(String),
  ClearAllBrowsingData,
  OnDestroy,
}

pub(crate) struct CreateWebViewAttributes {
  pub id: String,
  pub url: Option<String>,
  pub html: Option<String>,
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub devtools: bool,
  pub transparent: bool,
  pub background_color: Option<RGBA>,
  pub headers: Option<http::HeaderMap>,
  pub autoplay: bool,
  pub on_webview_created: Option<Box<dyn Fn(super::Context) -> JniResult<()> + Send>>,
  pub user_agent: Option<String>,
  pub initialization_scripts: Vec<(String, bool)>,
}

// SAFETY: only use this when you are sure the span will be dropped on the same thread it was entered
#[cfg(feature = "tracing")]
struct SendEnteredSpan(tracing::span::EnteredSpan);

#[cfg(feature = "tracing")]
unsafe impl Send for SendEnteredSpan {}
