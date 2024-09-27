use std::borrow::Cow;
use winit::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::Window,
};
use wry::{
  dpi::{LogicalPosition, LogicalSize},
  Rect, WebViewBuilder,
};

async fn run(event_loop: EventLoop<()>, window: Window) {
  let size = window.inner_size();

  let instance = wgpu::Instance::default();

  let surface = instance.create_surface(&window).unwrap();
  let adapter = instance
    .request_adapter(&wgpu::RequestAdapterOptions {
      power_preference: wgpu::PowerPreference::default(),
      force_fallback_adapter: false,
      // Request an adapter which can render to our surface
      compatible_surface: Some(&surface),
    })
    .await
    .expect("Failed to find an appropriate adapter");

  // Create the logical device and command queue
  let (device, queue) = adapter
    .request_device(
      &wgpu::DeviceDescriptor {
        label: None,
        required_features: wgpu::Features::empty(),
        // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
        required_limits: wgpu::Limits::downlevel_webgl2_defaults()
          .using_resolution(adapter.limits()),
      },
      None,
    )
    .await
    .expect("Failed to create device");

  // Load the shaders from disk
  let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: None,
    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(
      r#"
@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(in_vertex_index) - 1);
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
"#,
    )),
  });

  let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
    label: None,
    bind_group_layouts: &[],
    push_constant_ranges: &[],
  });

  let swapchain_capabilities = surface.get_capabilities(&adapter);
  let swapchain_format = swapchain_capabilities.formats[0];

  let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label: None,
    layout: Some(&pipeline_layout),
    vertex: wgpu::VertexState {
      module: &shader,
      entry_point: "vs_main",
      buffers: &[],
    },
    fragment: Some(wgpu::FragmentState {
      module: &shader,
      entry_point: "fs_main",
      targets: &[Some(swapchain_format.into())],
    }),
    primitive: wgpu::PrimitiveState::default(),
    depth_stencil: None,
    multisample: wgpu::MultisampleState::default(),
    multiview: None,
  });

  let mut config = wgpu::SurfaceConfiguration {
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
    format: swapchain_format,
    width: size.width,
    height: size.height,
    present_mode: wgpu::PresentMode::Fifo,
    desired_maximum_frame_latency: 2,
    alpha_mode: swapchain_capabilities.alpha_modes[0],
    view_formats: vec![],
  };

  surface.configure(&device, &config);

  let _webview = WebViewBuilder::new_as_child(&window)
    .with_bounds(Rect {
      position: LogicalPosition::new(100, 100).into(),
      size: LogicalSize::new(200, 200).into(),
    })
    .with_transparent(true)
    .with_html(
      r#"<html>
          <body style="background-color:rgba(87,87,87,0.5);"></body>
          <script>
            window.onload = function() {
              document.body.innerText = `hello, ${navigator.userAgent}`;
            };
          </script>
        </html>"#,
    )
    .build()
    .unwrap();

  event_loop
    .run(|event, evl| {
      evl.set_control_flow(ControlFlow::Poll);

      match event {
        Event::WindowEvent {
          event: WindowEvent::Resized(size),
          ..
        } => {
          // Reconfigure the surface with the new size
          config.width = size.width;
          config.height = size.height;
          surface.configure(&device, &config);
          // On macos the window needs to be redrawn manually after resizing
          window.request_redraw();
        }
        Event::WindowEvent {
          event: WindowEvent::RedrawRequested,
          ..
        } => {
          let frame = surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
          let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
          let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
          {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
              label: None,
              color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                  load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                  store: wgpu::StoreOp::Store,
                },
              })],
              depth_stencil_attachment: None,
              timestamp_writes: None,
              occlusion_query_set: None,
            });
            rpass.set_pipeline(&render_pipeline);
            rpass.draw(0..3, 0..1);
          }

          queue.submit(Some(encoder.finish()));
          frame.present();
        }
        Event::WindowEvent {
          event: WindowEvent::CloseRequested,
          ..
        } => evl.exit(),
        _ => {}
      }

      #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
      ))]
      while gtk::events_pending() {
        gtk::main_iteration_do(false);
      }
    })
    .unwrap();
}

fn main() {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  {
    use gtk::prelude::DisplayExtManual;

    gtk::init().unwrap();
    if gtk::gdk::Display::default().unwrap().backend().is_wayland() {
      panic!("This example doesn't support wayland!");
    }

    // we need to ignore this error here otherwise it will be catched by winit and will be
    // make the example crash
    winit::platform::x11::register_xlib_error_hook(Box::new(|_display, error| {
      let error = error as *mut x11_dl::xlib::XErrorEvent;
      (unsafe { (*error).error_code }) == 170
    }));
  }

  let event_loop = EventLoop::new().unwrap();
  let window = winit::window::WindowBuilder::new()
    .with_transparent(true)
    .build(&event_loop)
    .unwrap();
  pollster::block_on(run(event_loop, window));
}
