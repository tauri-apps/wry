// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::WebViewBuilder;

#[cfg(target_os = "windows")]
fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  use std::mem::ManuallyDrop;
  use windows::{
    core::{w, BSTR},
    Win32::System::{
      Com::{IDispatch, IDispatch_Impl, ITypeInfo, DISPATCH_FLAGS, DISPPARAMS, EXCEPINFO},
      Variant::{
        VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BSTR, VT_DISPATCH, VT_I4,
      },
    },
  };
  use wry::WebViewExtWindows;

  // This is a simple usage example. add_host_object_to_script is a mapping of the native [addHostObjectToScript](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2#addhostobjecttoscript) method of webview2. It requires manual creation of hostobject and memory management. Please use it with caution.
  struct Variant(VARIANT);
  impl Variant {
    pub fn new(num: VARENUM, contents: VARIANT_0_0_0) -> Variant {
      Variant {
        0: VARIANT {
          Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
              vt: num,
              wReserved1: 0,
              wReserved2: 0,
              wReserved3: 0,
              Anonymous: contents,
            }),
          },
        },
      }
    }
  }
  impl From<String> for Variant {
    fn from(value: String) -> Variant {
      Variant::new(
        VT_BSTR,
        VARIANT_0_0_0 {
          bstrVal: ManuallyDrop::new(BSTR::from(value)),
        },
      )
    }
  }
  impl From<&str> for Variant {
    fn from(value: &str) -> Variant {
      Variant::from(value.to_string())
    }
  }
  impl From<i32> for Variant {
    fn from(value: i32) -> Variant {
      Variant::new(VT_I4, VARIANT_0_0_0 { lVal: value })
    }
  }
  impl From<std::mem::ManuallyDrop<::core::option::Option<IDispatch>>> for Variant {
    fn from(value: std::mem::ManuallyDrop<::core::option::Option<IDispatch>>) -> Variant {
      Variant::new(VT_DISPATCH, VARIANT_0_0_0 { pdispVal: value })
    }
  }
  impl Drop for Variant {
    fn drop(&mut self) {
      match VARENUM(unsafe { self.0.Anonymous.Anonymous.vt.0 }) {
        VT_BSTR => unsafe { drop(&mut &self.0.Anonymous.Anonymous.Anonymous.bstrVal) },
        _ => {}
      }
      unsafe { drop(&mut self.0.Anonymous.Anonymous) }
    }
  }
  #[windows::core::implement(IDispatch)]
  struct FunctionWithStringArgument;
  impl IDispatch_Impl for FunctionWithStringArgument {
    #![allow(non_snake_case)]
    fn GetTypeInfoCount(&self) -> windows::core::Result<u32> {
      Ok(0)
    }
    fn GetTypeInfo(&self, _itinfo: u32, _lcid: u32) -> windows::core::Result<ITypeInfo> {
      Err(windows::core::Error::new(
        windows::Win32::Foundation::E_FAIL,
        "GetTypeInfo Error \t\n\r".into(),
      ))
    }
    fn GetIDsOfNames(
      &self,
      _riid: *const ::windows::core::GUID,
      _rgsznames: *const ::windows::core::PCWSTR,
      _cnames: u32,
      _lcid: u32,
      _rgdispid: *mut i32,
    ) -> windows::core::Result<()> {
      Ok(())
    }
    fn Invoke(
      &self,
      _dispidmember: i32,
      _riid: *const windows::core::GUID,
      _lcid: u32,
      _wflags: DISPATCH_FLAGS,
      pdispparams: *const DISPPARAMS,
      pvarresult: *mut VARIANT,
      _pexcepinfo: *mut EXCEPINFO,
      _puargerr: *mut u32,
    ) -> windows::core::Result<()> {
      let pdispparams = unsafe { *pdispparams };
      let rgvarg = unsafe { &*(pdispparams.rgvarg) };
      let rgvarg_0_0 = unsafe { &rgvarg.Anonymous.Anonymous };
      unsafe {
        dbg!(&rgvarg_0_0.Anonymous.bstrVal);
      }
      let b_str_val = unsafe { &rgvarg_0_0.Anonymous.bstrVal.to_string() };
      dbg!(b_str_val);

      let pvarresult_0_0 = unsafe { &mut (*pvarresult).Anonymous.Anonymous };
      pvarresult_0_0.vt = VT_BSTR;
      pvarresult_0_0.Anonymous.bstrVal = ManuallyDrop::new(BSTR::from(
        format!(
          r#"Successful sync call functionWithStringArgument, and the argument is "{}"."#,
          b_str_val
        )
        .to_string(),
      ));
      Ok(())
    }
  }
  let mut i32_variant = Variant::from(1234);
  let mut string_variant = Variant::from("string variant");
  let mut function_with_string_argument_variant = Variant::from(ManuallyDrop::new(Some(
    IDispatch::from(FunctionWithStringArgument),
  )));

  let webview = WebViewBuilder::new(&window)
    .with_url("https://tauri.app")?
    .with_initialization_script(
      r#"
      alert(chrome.webview.hostObjects.sync.i32)
      alert(chrome.webview.hostObjects.sync.string)
      alert(chrome.webview.hostObjects.sync.functionWithStringArgument("hi"))
    "#,
    )
    .build()?;

  unsafe {
    let _ = webview.add_host_object_to_script(w!("i32"), &mut i32_variant.0);
    let _ = webview.add_host_object_to_script(w!("string"), &mut string_variant.0);
    let _ = webview.add_host_object_to_script(
      w!("functionWithStringArgument"),
      &mut function_with_string_argument_variant.0,
    );
  }

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    if let Event::WindowEvent {
      event: WindowEvent::CloseRequested,
      ..
    } = event
    {
      *control_flow = ControlFlow::Exit
    }
  });
}

#[cfg(not(target_os = "windows"))]
fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let webview = WebViewBuilder::new(&window)
    .with_url("https://tauri.app")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    if let Event::WindowEvent {
      event: WindowEvent::CloseRequested,
      ..
    } = event
    {
      *control_flow = ControlFlow::Exit
    }
  });
}
