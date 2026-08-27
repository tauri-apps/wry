// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use wry::{
  dpi::{LogicalPosition, LogicalSize},
  Rect, WebViewBuilder,
};

fn main() -> wry::Result<()> {
  #[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  )))]
  linux_main()?;

  #[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  ))]
  non_linux_main()?;

  Ok(())
}

/// Linux implementation using GTK4 Application + GLArea directly.
#[cfg(not(any(
  target_os = "windows",
  target_os = "macos",
  target_os = "ios",
  target_os = "android"
)))]
fn linux_main() -> wry::Result<()> {
  use std::{cell::RefCell, rc::Rc};

  use gtk4::prelude::*;
  use wry::WebViewBuilderExtUnix;

  gtk4::init().unwrap();

  let app = gtk4::Application::new(
    Some("com.example.gtk_opengl"),
    gtk4::gio::ApplicationFlags::empty(),
  );

  app.connect_activate(|app| {
    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some("GTK OpenGL with Webview"));
    window.set_default_size(800, 600);

    let overlay = gtk4::Overlay::new();
    window.set_child(Some(&overlay));

    let gl_area = gtk4::GLArea::new();
    gl_area.set_auto_render(true);
    gl_area.set_has_depth_buffer(false);
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);

    struct AppState {
      gl: glow::Context,
      program: glow::Program,
      vertex_array: glow::VertexArray,
    }

    let state: Rc<RefCell<Option<AppState>>> = Rc::new(RefCell::new(None));
    let state_realize = state.clone();

    gl_area.connect_realize(move |gl_area| {
      gl_area.make_current();
      if gl_area.error().is_some() {
        println!("Error creating GLArea context");
        return;
      }

      let gl = unsafe {
        glow::Context::from_loader_function(|s| {
          let mut ptr = std::ptr::null();
          let name = std::ffi::CString::new(s).unwrap();

          if let Ok(lib) = libloading::Library::new("libGL.so.1") {
            if let Ok(sym) = lib.get::<unsafe extern "C" fn(*const i8) -> *const std::ffi::c_void>(
              b"glXGetProcAddress\0",
            ) {
              ptr = sym(name.as_ptr());
            }
          }
          if ptr.is_null() {
            if let Ok(lib) = libloading::Library::new("libEGL.so.1") {
              if let Ok(sym) = lib
                .get::<unsafe extern "C" fn(*const i8) -> *const std::ffi::c_void>(
                  b"eglGetProcAddress\0",
                )
              {
                ptr = sym(name.as_ptr());
              }
            }
          }
          ptr
        })
      };

      unsafe {
        use glow::HasContext as _;

        let vertex_array = gl.create_vertex_array().unwrap();
        gl.bind_vertex_array(Some(vertex_array));

        let program = gl.create_program().expect("Cannot create program");

        let vertex_shader_source = r#"
        #version 330 core
        void main() {
            if (gl_VertexID == 0) gl_Position = vec4(-0.5, -0.5, 0.0, 1.0);
            else if (gl_VertexID == 1) gl_Position = vec4(0.5, -0.5, 0.0, 1.0);
            else gl_Position = vec4(0.0, 0.5, 0.0, 1.0);
        }
        "#;

        let fragment_shader_source = r#"
        #version 330 core
        out vec4 FragColor;
        void main() {
            FragColor = vec4(1.0, 0.5, 0.2, 1.0);
        }
        "#;

        let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vs, vertex_shader_source);
        gl.compile_shader(vs);
        if !gl.get_shader_compile_status(vs) {
          panic!("{}", gl.get_shader_info_log(vs));
        }

        let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fs, fragment_shader_source);
        gl.compile_shader(fs);
        if !gl.get_shader_compile_status(fs) {
          panic!("{}", gl.get_shader_info_log(fs));
        }

        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
          panic!("{}", gl.get_program_info_log(program));
        }

        gl.detach_shader(program, vs);
        gl.delete_shader(vs);
        gl.detach_shader(program, fs);
        gl.delete_shader(fs);

        *state_realize.borrow_mut() = Some(AppState {
          gl,
          program,
          vertex_array,
        });
      }
    });

    let state_render = state.clone();
    gl_area.connect_render(move |_gl_area, _gl_context| {
      if let Some(state) = state_render.borrow().as_ref() {
        unsafe {
          use glow::HasContext as _;
          state.gl.clear_color(0.1, 0.2, 0.3, 1.0);
          state.gl.clear(glow::COLOR_BUFFER_BIT);

          state.gl.use_program(Some(state.program));
          state.gl.bind_vertex_array(Some(state.vertex_array));
          state.gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
      }
      gtk4::glib::Propagation::Proceed
    });

    gl_area.connect_unrealize(move |gl_area| {
      gl_area.make_current();
      if let Some(state) = state.borrow_mut().take() {
        unsafe {
          use glow::HasContext as _;
          state.gl.delete_program(state.program);
          state.gl.delete_vertex_array(state.vertex_array);
        }
      }
    });

    // The gl_area goes as the base child of the overlay
    overlay.set_child(Some(&gl_area));

    let fixed = gtk4::Fixed::new();
    overlay.add_overlay(&fixed);

    let webview = WebViewBuilder::new()
      .with_bounds(Rect {
        position: LogicalPosition::new(100, 100).into(),
        size: LogicalSize::new(400, 300).into(),
      })
      .with_transparent(true)
      .with_html(
        r#"<html>
          <body>
            <h1 style="color: white;">Hello World!</h1>
          </body>
      </html>"#,
      )
      .build_gtk(&fixed)
      .unwrap();

    window.present();

    // Keep the webview alive for the window's lifetime.
    let webview = std::cell::RefCell::new(Some(webview));
    window.connect_close_request(move |_| {
      webview.borrow_mut().take();
      gtk4::glib::Propagation::Proceed
    });
  });

  app.run();
  Ok(())
}

/// Non-Linux placeholder — GTK OpenGL is a Linux-only example.
#[cfg(any(
  target_os = "windows",
  target_os = "macos",
  target_os = "ios",
  target_os = "android"
))]
fn non_linux_main() -> wry::Result<()> {
  eprintln!("gtk_opengl example is only supported on Linux.");
  Ok(())
}
