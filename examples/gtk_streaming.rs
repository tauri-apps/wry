// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Streaming custom protocol example using the GTK4 backend.
//!
//! Serves a local HTML page and handles HTTP Range requests for a video file
//! via a custom `stream://` protocol.
//!
//! Run with:
//!
//! ```bash
//! cargo run --features protocol --example gtk_streaming
//! ```
//!
//! See LINUX.md for troubleshooting tips.

fn main() -> wry::Result<()> {
  imp::main()
}

#[cfg(not(feature = "protocol"))]
mod imp {
  pub fn main() -> wry::Result<()> {
    unimplemented!("rerun with --features protocol")
  }
}

#[cfg(feature = "protocol")]
mod imp {
  use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
  };

  use http::{header, StatusCode};
  use http_range::HttpRange;
  use wry::http::{header::*, Request, Response};

  pub fn main() -> wry::Result<()> {
    #[cfg(any(
      target_os = "linux",
      target_os = "dragonfly",
      target_os = "freebsd",
      target_os = "netbsd",
      target_os = "openbsd",
    ))]
    return linux_main();

    #[cfg(not(any(
      target_os = "linux",
      target_os = "dragonfly",
      target_os = "freebsd",
      target_os = "netbsd",
      target_os = "openbsd",
    )))]
    {
      eprintln!("gtk_streaming is a Linux/BSD-only example.");
      Ok(())
    }
  }

  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  fn linux_main() -> wry::Result<()> {
    use std::cell::RefCell;

    use gtk4::prelude::*;
    use wry::WebViewBuilderExtUnix;

    let app = gtk4::Application::new(None::<&str>, Default::default());

    app.connect_activate(|app| {
      let window = gtk4::ApplicationWindow::new(app);
      window.set_title(Some("Streaming (GTK4 / Wayland)"));
      window.set_default_size(800, 600);

      let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
      window.set_child(Some(&vbox));
      window.present();

      let webview = wry::WebViewBuilder::new()
        .with_custom_protocol("wry".into(), move |_webview_id, request| {
          match wry_protocol(request) {
            Ok(r) => r.map(Into::into),
            Err(e) => http::Response::builder()
              .header(CONTENT_TYPE, "text/plain")
              .status(500)
              .body(e.to_string().as_bytes().to_vec())
              .unwrap()
              .map(Into::into),
          }
        })
        .with_custom_protocol("stream".into(), move |_webview_id, request| {
          match stream_protocol(request) {
            Ok(r) => r.map(Into::into),
            Err(e) => http::Response::builder()
              .header(CONTENT_TYPE, "text/plain")
              .status(500)
              .body(e.to_string().as_bytes().to_vec())
              .unwrap()
              .map(Into::into),
          }
        })
        .with_url("wry://localhost")
        .build_gtk(&vbox)
        .unwrap();

      let webview = RefCell::new(Some(webview));
      window.connect_close_request(move |_| {
        webview.borrow_mut().take();
        gtk4::glib::Propagation::Proceed
      });
    });

    app.run();
    Ok(())
  }

  fn wry_protocol(
    request: Request<Vec<u8>>,
  ) -> Result<http::Response<Vec<u8>>, Box<dyn std::error::Error>> {
    let path = request.uri().path();
    let root = PathBuf::from("examples/streaming");
    let path = if path == "/" { "index.html" } else { &path[1..] };
    let content = std::fs::read(std::fs::canonicalize(root.join(path))?)?;

    let mimetype = if path.ends_with(".html") || path == "/" {
      "text/html"
    } else if path.ends_with(".js") {
      "text/javascript"
    } else {
      unimplemented!();
    };

    Response::builder()
      .header(CONTENT_TYPE, mimetype)
      .body(content)
      .map_err(Into::into)
  }

  fn video_mime(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
      .extension()
      .and_then(|e| e.to_str())
      .unwrap_or("")
      .to_ascii_lowercase();

    match ext.as_str() {
      // MPEG-4 container
      "mp4" | "m4v" | "m4p" => "video/mp4",
      // WebM (VP8 / VP9 / AV1)
      "webm" => "video/webm",
      // Ogg (Theora)
      "ogg" | "ogv" => "video/ogg",
      // QuickTime
      "mov" | "qt" => "video/quicktime",
      // Matroska / MKV
      "mkv" | "mk3d" => "video/x-matroska",
      // AVI
      "avi" => "video/x-msvideo",
      // Flash video
      "flv" | "f4v" => "video/x-flv",
      // Windows Media
      "wmv" | "asf" => "video/x-ms-wmv",
      // MPEG-1 / MPEG-2
      "mpeg" | "mpg" | "mpe" | "m2v" | "m1v" => "video/mpeg",
      // MPEG-2 Transport Stream
      "ts" | "m2ts" | "mts" => "video/mp2t",
      // 3GPP (mobile)
      "3gp" | "3gpp" => "video/3gpp",
      "3g2" | "3gpp2" => "video/3gpp2",
      // HEVC / H.265 in raw annex-b form
      "hevc" | "h265" => "video/hevc",
      // AVCHD / Blu-ray disc clip
      "mxf" => "application/mxf",
      // RealMedia
      "rm" | "rmvb" => "application/vnd.rn-realmedia",
      // fallback — assume MP4 container
      _ => "video/mp4",
    }
  }

  fn stream_protocol(
    request: http::Request<Vec<u8>>,
  ) -> Result<http::Response<Vec<u8>>, Box<dyn std::error::Error>> {
    let path = percent_encoding::percent_decode(&request.uri().path().as_bytes()[1..])
      .decode_utf8_lossy()
      .to_string();

    let mut file = std::fs::File::open(&path)?;

    let len = {
      let old_pos = file.stream_position()?;
      let len = file.seek(SeekFrom::End(0))?;
      file.seek(SeekFrom::Start(old_pos))?;
      len
    };

    let mut resp = Response::builder().header(CONTENT_TYPE, video_mime(&path));

    // Only macOS and Windows send Range headers; Linux always returns empty headers.
    let http_response = if let Some(range_header) = request.headers().get("range") {
      let not_satisfiable = || {
        Response::builder()
          .status(StatusCode::RANGE_NOT_SATISFIABLE)
          .header(header::CONTENT_RANGE, format!("bytes */{len}"))
          .body(vec![])
      };

      let ranges = if let Ok(ranges) = HttpRange::parse(range_header.to_str()?, len) {
        ranges
          .iter()
          .map(|r| (r.start, r.start + r.length - 1))
          .collect::<Vec<_>>()
      } else {
        return Ok(not_satisfiable()?);
      };

      const MAX_LEN: u64 = 1000 * 1024;

      if ranges.len() == 1 {
        let &(start, mut end) = ranges.first().unwrap();

        if start >= len || end >= len || end < start {
          return Ok(not_satisfiable()?);
        }

        end = start + (end - start).min(len - start).min(MAX_LEN - 1);
        let bytes_to_read = end + 1 - start;

        let mut buf = Vec::with_capacity(bytes_to_read as usize);
        file.seek(SeekFrom::Start(start))?;
        file.take(bytes_to_read).read_to_end(&mut buf)?;

        resp = resp.header(CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
        resp = resp.header(CONTENT_LENGTH, end + 1 - start);
        resp = resp.status(StatusCode::PARTIAL_CONTENT);
        resp.body(buf)
      } else {
        let mut buf = Vec::new();
        let ranges = ranges
          .iter()
          .filter_map(|&(start, mut end)| {
            if start >= len || end >= len || end < start {
              None
            } else {
              end = start + (end - start).min(len - start).min(MAX_LEN - 1);
              Some((start, end))
            }
          })
          .collect::<Vec<_>>();

        let boundary = random_boundary();
        let boundary_sep = format!("\r\n--{boundary}\r\n");
        let boundary_closer = format!("\r\n--{boundary}\r\n");

        resp = resp.header(
          CONTENT_TYPE,
          format!("multipart/byteranges; boundary={boundary}"),
        );

        for (end, start) in ranges {
          buf.write_all(boundary_sep.as_bytes())?;
          buf.write_all(format!("{CONTENT_TYPE}: {}\r\n", video_mime(&path)).as_bytes())?;
          buf.write_all(format!("{CONTENT_RANGE}: bytes {start}-{end}/{len}\r\n").as_bytes())?;
          buf.write_all("\r\n".as_bytes())?;

          let bytes_to_read = end + 1 - start;
          let mut local_buf = vec![0_u8; bytes_to_read as usize];
          file.seek(SeekFrom::Start(start))?;
          file.read_exact(&mut local_buf)?;
          buf.extend_from_slice(&local_buf);
        }
        buf.write_all(boundary_closer.as_bytes())?;

        resp.body(buf)
      }
    } else {
      resp = resp.header(CONTENT_LENGTH, len);
      let mut buf = Vec::with_capacity(len as usize);
      file.read_to_end(&mut buf)?;
      resp.body(buf)
    };

    http_response.map_err(Into::into)
  }

  fn random_boundary() -> String {
    let mut x = [0_u8; 30];
    getrandom::fill(&mut x).expect("failed to get random bytes");
    x.iter()
      .map(|&b| format!("{b:x}"))
      .fold(String::new(), |mut a, x| {
        a.push_str(x.as_str());
        a
      })
  }
}
