use crossbeam_channel::*;
use once_cell::sync::Lazy;
use std::os::unix::prelude::*;
use tao::platform::android::ndk_glue::{
  jni::{
    errors::Error as JniError,
    objects::{GlobalRef, JClass, JObject},
    JNIEnv,
  },
  PACKAGE,
};

static CHANNEL: Lazy<(Sender<WebViewMessage>, Receiver<WebViewMessage>)> = Lazy::new(|| bounded(8));
pub static MAIN_PIPE: Lazy<[RawFd; 2]> = Lazy::new(|| {
  let mut pipe: [RawFd; 2] = Default::default();
  unsafe { libc::pipe(pipe.as_mut_ptr()) };
  pipe
});

pub struct MainPipe<'a> {
  pub env: JNIEnv<'a>,
  pub activity: GlobalRef,
  pub initialization_scripts: Vec<String>,
  pub webview: Option<GlobalRef>,
}

impl MainPipe<'_> {
  pub fn send(message: WebViewMessage) {
    let size = std::mem::size_of::<bool>();
    if let Ok(()) = CHANNEL.0.send(message) {
      unsafe { libc::write(MAIN_PIPE[1], &true as *const _ as *const _, size) };
    }
  }

  pub fn recv(&mut self) -> Result<(), JniError> {
    let env = self.env;
    let activity = self.activity.as_obj();
    if let Ok(message) = CHANNEL.1.recv() {
      match message {
        WebViewMessage::CreateWebView(url, mut initialization_scripts, devtools) => {
          // Create webview
          let class = env.find_class("android/webkit/WebView")?;
          let webview =
            env.new_object(class, "(Landroid/content/Context;)V", &[activity.into()])?;

          // Enable Javascript
          let settings = env
            .call_method(
              webview,
              "getSettings",
              "()Landroid/webkit/WebSettings;",
              &[],
            )?
            .l()?;
          env.call_method(settings, "setJavaScriptEnabled", "(Z)V", &[true.into()])?;

          // Load URL
          if let Ok(url) = env.new_string(url) {
            env.call_method(webview, "loadUrl", "(Ljava/lang/String;)V", &[url.into()])?;
          }

          // Enable devtools
          env.call_static_method(
            class,
            "setWebContentsDebuggingEnabled",
            "(Z)V",
            &[devtools.into()],
          )?;

          // Initialize scripts
          self
            .initialization_scripts
            .append(&mut initialization_scripts);

          // Create and set webview client
          println!(
            "[RUST] webview client {}/RustWebViewClient",
            PACKAGE.get().unwrap()
          );
          let rust_webview_client_class = find_my_class(
            env,
            activity,
            format!("{}/RustWebViewClient", PACKAGE.get().unwrap()),
          )?;
          let webview_client = env.new_object(rust_webview_client_class, "()V", &[])?;
          env.call_method(
            webview,
            "setWebViewClient",
            "(Landroid/webkit/WebViewClient;)V",
            &[webview_client.into()],
          )?;

          // Create and set webchrome client
          println!("[RUST] chrome client");
          let rust_webchrome_client_class = find_my_class(
            env,
            activity,
            format!("{}/RustWebChromeClient", PACKAGE.get().unwrap()),
          )?;
          let webchrome_client = env.new_object(rust_webchrome_client_class, "()V", &[])?;
          env.call_method(
            webview,
            "setWebChromeClient",
            "(Landroid/webkit/WebChromeClient;)V",
            &[webchrome_client.into()],
          )?;

          // Add javascript interface (IPC)
          let ipc_class = find_my_class(env, activity, format!("{}/Ipc", PACKAGE.get().unwrap()))?;
          let ipc = env.new_object(ipc_class, "()V", &[])?;
          let ipc_str = env.new_string("ipc")?;
          env.call_method(
            webview,
            "addJavascriptInterface",
            "(Ljava/lang/Object;Ljava/lang/String;)V",
            &[ipc.into(), ipc_str.into()],
          )?;

          // Set content view
          env.call_method(
            activity,
            "setContentView",
            "(Landroid/view/View;)V",
            &[webview.into()],
          )?;
          let webview = env.new_global_ref(webview)?;
          self.webview = Some(webview);
        }
        WebViewMessage::RunInitializationScripts => {
          if let Some(webview) = &self.webview {
            for s in &self.initialization_scripts {
              let s = env.new_string(s)?;
              env.call_method(
                webview.as_obj(),
                "evaluateJavascript",
                "(Ljava/lang/String;Landroid/webkit/ValueCallback;)V",
                &[s.into(), JObject::null().into()],
              )?;
            }
          }
        }
        WebViewMessage::Eval(script) => {
          if let Some(webview) = &self.webview {
            let s = env.new_string(script)?;
            env.call_method(
              webview.as_obj(),
              "evaluateJavascript",
              "(Ljava/lang/String;Landroid/webkit/ValueCallback;)V",
              &[s.into(), JObject::null().into()],
            )?;
          }
        }
      }
    }
    Ok(())
  }
}

fn find_my_class<'a>(
  env: JNIEnv<'a>,
  activity: JObject<'a>,
  name: String,
) -> Result<JClass<'a>, JniError> {
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

#[derive(Debug)]
pub enum WebViewMessage {
  CreateWebView(String, Vec<String>, bool),
  RunInitializationScripts,
  Eval(String),
}
