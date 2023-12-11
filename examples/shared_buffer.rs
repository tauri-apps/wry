// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, WindowEvent},
  event_loop::ControlFlow,
  window::WindowBuilder,
};
use wry::WebViewBuilder;

// Currently, only Windows platforms support shared_buffer.
#[cfg(target_os = "windows")]
fn main() -> wry::Result<()> {
  use wry::WebViewExtWindows;

  enum UserEvent {
    InitSharedBuffer,
    PingSharedBuffer,
  }

  let event_loop = tao::event_loop::EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let webview = WebViewBuilder::new(&window)
    .with_url("https://tauri.app")?
    .with_ipc_handler(move |req: String| match req.as_str() {
      "initSharedBuffer" => { let _ = proxy.send_event(UserEvent::InitSharedBuffer); }
      "pingSharedBuffer" => { let _ = proxy.send_event(UserEvent::PingSharedBuffer); }
      _ => {}
    })
    .with_initialization_script(r#";(function() {
      function writeStringIntoSharedBuffer(string, sharedBuffer, pathPtr) {
        const path = new TextEncoder().encode(string)
        const pathLen = path.length
        const pathArray = new Uint8Array(sharedBuffer, pathPtr, pathLen*8)
        for(let i = 0; i < pathLen; i++) {
          pathArray[i] = path[i]
        }
        return [pathPtr, pathLen]
      }

      const sharedBufferReceivedHandler = e => {
        window.chrome.webview.removeEventListener("sharedbufferreceived", sharedBufferReceivedHandler);

        alert(JSON.stringify(e.additionalData))

        var sharedBuffer = e.getBuffer()
        console.log(sharedBuffer)
        window.sharedBuffer = sharedBuffer

        // JS write
        writeStringIntoSharedBuffer("I'm JS.", sharedBuffer, 0)

        window.ipc.postMessage('pingSharedBuffer');
      }
      window.chrome.webview.addEventListener("sharedbufferreceived", sharedBufferReceivedHandler);
      window.ipc.postMessage('initSharedBuffer');
    })();"#)
    .build()?;

  // The Webview2 developer tools include a memory inspector, which makes it easy to debug memory issues.
  webview.open_devtools();

  let mut shared_buffer: Option<
    webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2SharedBuffer,
  > = None;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;
    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => {
        *control_flow = ControlFlow::Exit
      },

      Event::UserEvent(e) => match e {
        UserEvent::InitSharedBuffer => {
          // Memory obtained through webview2 must be manually managed. Use it with care.
          shared_buffer = Some(unsafe { webview.create_shared_buffer(1024) }.unwrap());
          if let Some(shared_buffer) = &shared_buffer {
            dbg!(shared_buffer);
            let _ = unsafe {
              webview.post_shared_buffer_to_script(
                shared_buffer,
                webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE,
                windows::core::w!(r#"{"jsonkey":"jsonvalue"}"#)
              )
            };
          }
        },
        UserEvent::PingSharedBuffer => {
          if let Some(shared_buffer) = &shared_buffer {
            let mut ptr: *mut u8 = &mut 0u8;
            let _ = unsafe { shared_buffer.Buffer(&mut ptr) };

            // Rust read
            let len = 8; // align to 4
            let read_string: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
            let read_string = std::str::from_utf8(&read_string).unwrap();
            dbg!(read_string);

            // Rust write
            let mut vec = String::from("I'm Rust.").into_bytes();
            unsafe { std::ptr::copy((&mut vec).as_mut_ptr(), ptr.offset(len as isize), 9) };

            let _ = webview.evaluate_script(r#";(function() {
              // JS read
              alert(
                new TextDecoder()
                .decode(new Uint8Array(window.sharedBuffer, 8, 9))
              )
            })()"#);
          }
        }
      },

      _ => (),
    }
  });
}

// Non-Windows systems do not yet support shared_buffer.
#[cfg(not(target_os = "windows"))]
fn main() -> wry::Result<()> {
  let event_loop = tao::event_loop::EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let _ = WebViewBuilder::new(&window)
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
