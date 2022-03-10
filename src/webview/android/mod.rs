use std::{ffi::c_void, ptr::null_mut, rc::Rc, sync::RwLock};

use crate::{application::window::Window, Result};

use super::{WebContext, WebViewAttributes};

use jni::{
  objects::{JClass, JObject},
  sys::jobject,
  JNIEnv,
};

use once_cell::sync::Lazy;

static IPC: Lazy<RwLock<UnsafeIpc>> = Lazy::new(|| RwLock::new(UnsafeIpc(null_mut())));

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

  pub fn print(&self) {}

  pub fn eval(&self, _js: &str) -> Result<()> {
    Ok(())
  }

  pub fn focus(&self) {}

  pub fn devtool(&self) {}

  pub fn run(self, env: JNIEnv, _jclass: JClass, jobject: JObject) -> Result<jobject> {
    let string_class = env.find_class("java/lang/String")?;
    // let client = env.call_method(
    //   jobject,
    //   "getWebViewClient",
    //   "()Landroid/webkit/WebViewClient;",
    //   &[],
    // )?;
    let WebViewAttributes {
      url,
      initialization_scripts,
      ipc_handler,
      ..
    } = self.attributes;

    // todo
    // ipc too?
    if let Some(i) = ipc_handler {
      let i = UnsafeIpc(Box::into_raw(Box::new(i)) as *mut _);
      let mut ipc = IPC.write().unwrap();
      *ipc = i;
    }

    if let Some(u) = url {
      let url = env.new_string(u)?;
      env.call_method(jobject, "loadUrl", "(Ljava/lang/String;)V", &[url.into()])?;
    }

    // Return initialization scripts
    let len = initialization_scripts.len();
    let scripts = env.new_object_array(len as i32, string_class, env.new_string("")?)?;
    for (idx, s) in initialization_scripts.into_iter().enumerate() {
      env.set_object_array_element(scripts, idx as i32, env.new_string(s)?)?;
    }
    Ok(scripts)
  }

  pub fn ipc_handler(window: &Window, arg: String) {
    let function = IPC.read().unwrap();
    unsafe {
      let ipc = function.0;
      if !ipc.is_null() {
        let ipc = &*(ipc as *mut Box<dyn Fn(&Window, String)>);
        ipc(window, arg)
      }
    }
  }
}

pub struct UnsafeIpc(*mut c_void);
unsafe impl Send for UnsafeIpc {}
unsafe impl Sync for UnsafeIpc {}

pub fn platform_webview_version() -> Result<String> {
  todo!()
}
