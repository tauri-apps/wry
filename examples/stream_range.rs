// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use http_range::HttpRange;
  use std::{
    fs::{canonicalize, File},
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    process::{Command, Stdio},
  };
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    http::ResponseBuilder,
    webview::WebViewBuilder,
  };

  let video_file = PathBuf::from("examples/test_video.mp4");
  let video_url =
    "http://distribution.bbb3d.renderfarming.net/video/mp4/bbb_sunflower_1080p_30fps_normal.mp4";

  if !video_file.exists() {
    // Downloading with curl this saves us from adding
    // a Rust HTTP client dependency.
    println!("Downloading {}", video_url);
    let status = Command::new("curl")
      .arg("-L")
      .arg("-o")
      .arg(&video_file)
      .arg(video_url)
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .output()
      .unwrap();

    assert!(status.status.success());
    assert!(video_file.exists());
  }

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();

  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_custom_protocol("wry".into(), move |request| {
      // Remove url scheme
      let path = request.uri().replace("wry://", "");
      // Read the file content from file path
      let mut content = File::open(canonicalize(&path)?)?;

      // Return asset contents and mime types based on file extentions
      // If you don't want to do this manually, there are some crates for you.
      // Such as `infer` and `mime_guess`.
      let mut status_code = 200;
      let mut buf = Vec::new();

      // guess our mimetype from the path
      let mimetype = if path.ends_with(".html") {
        "text/html"
      } else if path.ends_with(".mp4") {
        "video/mp4"
      } else {
        unimplemented!();
      };

      // prepare our http response
      let mut response = ResponseBuilder::new();

      // read our range header if it exist, so we can return partial content
      if let Some(range) = request.headers().get("range") {
        // Get the file size
        let file_size = content.metadata().unwrap().len();

        // we parse the range header
        let range = HttpRange::parse(range.to_str().unwrap(), file_size).unwrap();

        // let support only 1 range for now
        let first_range = range.first();
        if let Some(range) = first_range {
          let mut real_length = range.length;

          // prevent max_length;
          // specially on webview2
          if range.length > file_size / 3 {
            // max size sent (400ko / request)
            // as it's local file system we can afford to read more often
            real_length = 1024 * 400;
          }

          // last byte we are reading, the length of the range include the last byte
          // who should be skipped on the header
          let last_byte = range.start + real_length - 1;
          // partial content
          status_code = 206;

          response = response.header("Connection", "Keep-Alive");
          response = response.header("Accept-Ranges", "bytes");
          // we need to overwrite our content length
          response = response.header("Content-Length", real_length);
          response = response.header(
            "Content-Range",
            format!("bytes {}-{}/{}", range.start, last_byte, file_size),
          );

          // seek our file bytes
          content.seek(SeekFrom::Start(range.start))?;
          content.take(real_length).read_to_end(&mut buf)?;
        } else {
          content.read_to_end(&mut buf)?;
        }
      } else {
        content.read_to_end(&mut buf)?;
      }

      response.mimetype(mimetype).status(status_code).body(buf)
    })
    // tell the webview to load the custom protocol
    .with_url("wry://examples/stream.html")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry application started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => {
        #[cfg(target_os = "windows")]
        let _ = _webview.resize();
      }
    }
  });
}
