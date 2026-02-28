// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::cell::RefCell;
use std::ffi::CString;
use std::rc::Rc;
use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::{
  dpi::{LogicalPosition, LogicalSize},
  Rect, WebViewBuilder,
};

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("GTK OpenGL with Webview")
    .with_inner_size(LogicalSize::new(800, 600))
    .build(&event_loop)
    .unwrap();

  #[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  )))]
  let (fixed, gl_area) = {
    use gtk::prelude::*;
    use tao::platform::unix::WindowExtUnix;
    let fixed = gtk::Fixed::new();
    let vbox = window.default_vbox().unwrap();
    vbox.pack_start(&fixed, true, true, 0);

    let gl_area = gtk::GLArea::new();
    gl_area.set_has_alpha(true);
    gl_area.set_auto_render(true);

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
        let lib = Box::leak(Box::new(
          libloading::Library::new("libepoxy.so.0").expect("Failed to load libepoxy"),
        ));
        glow::Context::from_loader_function_unsafe(|s| {
          let name = CString::new(s).unwrap();
          let get_proc_address: libloading::Symbol<
            unsafe extern "C" fn(*const i8) -> *const std::ffi::c_void,
          > = lib.get(b"epoxy_get_proc_address\0").unwrap();
          get_proc_address(name.as_ptr())
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
    gl_area.connect_render(move |gl_area, _gl_context| {
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
      gtk::Inhibit(false)
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

    gl_area.set_size_request(800, 600);
    fixed.put(&gl_area, 0, 0);

    fixed.show_all();
    (fixed, gl_area)
  };

  let builder = WebViewBuilder::new()
    .with_bounds(Rect {
      position: LogicalPosition::new(100, 100).into(),
      size: LogicalSize::new(400, 300).into(),
    })
    .with_transparent(true)
    .with_html(
      r#"<html>
          <body style="background-color:rgba(87,87,87,0.5); color: white; display: flex; justify-content: center; align-items: center;">
            <h1>Webview Overlay Layer</h1>
          </body>
      </html>"#,
    );

  #[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  ))]
  let webview = builder.build(&window)?;

  #[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  )))]
  let webview = {
    use wry::WebViewBuilderExtUnix;
    builder.build_gtk(&fixed)?
  };

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::WindowEvent {
        event: WindowEvent::Resized(size),
        ..
      } => {
        let size = size.to_logical::<u32>(window.scale_factor());
        webview
          .set_bounds(Rect {
            position: LogicalPosition::new(100, 100).into(),
            size: LogicalSize::new(
              size.width.saturating_sub(200).max(100),
              size.height.saturating_sub(200).max(100),
            )
            .into(),
          })
          .unwrap();

        #[cfg(not(any(
          target_os = "windows",
          target_os = "macos",
          target_os = "ios",
          target_os = "android"
        )))]
        {
          use gtk::prelude::*;
          gl_area.set_size_request(size.width as i32, size.height as i32);
        }
      }
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => {}
    }
  });
}
