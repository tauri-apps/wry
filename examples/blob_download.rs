// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{path::PathBuf, fs::File, io::{Write, Read}};

use base64::decode;
use tempfile::tempdir;

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  let html = r#"
    <body>
      <div>
        <a download="hello.txt" href='#' id="link">Download</a>
        <script>
        const example = new Blob(["Hello, world!"], {type: 'text/plain'});
        link.href = URL.createObjectURL(example);
        </script>
      </div>
    </body>
  "#;

  enum UserEvent {
    BlobReceived(String),
    BlobChunk(Option<String>),
  }

  let init_script = r"
    // Adds an URL.getFromObjectURL( <blob:// URI> ) method
    // returns the original object (<Blob> or <MediaSource>) the URI points to or null
    (() => {
      // overrides URL methods to be able to retrieve the original blobs later on
      const old_create = URL.createObjectURL;
      const old_revoke = URL.revokeObjectURL;
      Object.defineProperty(URL, 'createObjectURL', {
        get: () => storeAndCreate
      });
      Object.defineProperty(URL, 'revokeObjectURL', {
        get: () => forgetAndRevoke
      });
      Object.defineProperty(URL, 'getFromObjectURL', {
        get: () => getBlob
      });
      Object.defineProperty(URL, 'getObjectURLDict', {
        get: () => getDict
      });
      Object.defineProperty(URL, 'clearURLDict', {
        get: () => clearDict
      });
      const dict = {};
      
      function storeAndCreate(blob) {
        const url = old_create(blob); // let it throw if it has to
        dict[url] = blob;
        console.log(url)
        console.log(blob)
        return url
      }
      
      function forgetAndRevoke(url) {
        console.log(`revoke ${url}`)
        old_revoke(url);
      }
      
      function getBlob(url) {
        return dict[url] || null;
      }
      
      function getDict() {
        return dict;
      }
      
      function clearDict() {
        dict = {};
      }
    })();
  ";

  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let webview = WebViewBuilder::new(window)?
    .with_html(html)?
    .with_initialization_script(init_script)
    .with_download_handler({
      let proxy = proxy.clone();
      move |uri: String, _: &mut PathBuf| {
        if uri.starts_with("blob:") {
          let _ = proxy.send_event(UserEvent::BlobReceived(dbg!(uri)));
        }

        false
      }
    })
    .with_ipc_handler({
      let proxy = proxy.clone();
      move |_, string| match string.as_str() {
        _ if string.starts_with("data:") => {
          let _ = proxy.send_event(UserEvent::BlobChunk(Some(string)));
        }
        "#EOF" => {
          let _ = proxy.send_event(UserEvent::BlobChunk(None));
        }
        _ => {}
      }
    })
    .with_devtools(true)
    .build()?;

  #[cfg(debug_assertions)]
  webview.open_devtools();

  let mut blob_file = None;
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(UserEvent::BlobReceived(uri)) => {
        let temp_dir = tempdir().expect("Create temp dir");
        blob_file = Some((File::create(&temp_dir.path().join("blob.txt")).expect("Create file"), temp_dir));
        webview.evaluate_script(&format!(r#"
        (() => {{
          /**
          * @type Blob
          */
          let blob = URL.getObjectURLDict()['{}']
            || Object.values(URL.getObjectURLDict())[0]   // For some reason macOS returns a completely random blob URL? Just grab the first one

          var increment = 1024;
          var index = 0;
          var reader = new FileReader();
          let func = function() {{
            let res = reader.result;
            window.ipc.postMessage(`${{res}}`);
            index += increment;
            if (index < blob.size) {{
              let slice = blob.slice(index, index + increment);
              reader = new FileReader();
              reader.onloadend = func;
              reader.readAsDataURL(slice);
            }} else {{
              window.ipc.postMessage('#EOF');
            }}
          }};
          reader.onloadend = func;
          reader.readAsDataURL(blob.slice(index, increment))
        }})();
        "#, uri)).expect("Eval script");
      },
      Event::UserEvent(UserEvent::BlobChunk(chunk)) => {
        if let Some((file, path)) = blob_file.as_mut() {
          match chunk {
            Some(chunk) => {
              let split = chunk.split(',').nth(1);
              println!("{:?}", chunk.split(',').next());
              if let Some(split) = split {
                if let Ok(decoded) = decode(split) {
                  if file.write(&decoded).is_err() {
                    eprintln!("Failed to write bytes to temp file")
                  }
                }
              }
            },
            None => {
              let mut file = File::open(&path.path().join("blob.txt")).expect("Open temp file");
              let mut content = String::new();
              file.read_to_string(&mut content).expect("Read contents of file");
              println!("Contents of file:");
              println!("{}", content);
              blob_file = None;
            }
          }
        }
      },
      _ => (),
    }
  });
}
