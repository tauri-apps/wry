use wry::platform::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::webview::*;

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let webview = WebViewBuilder::new(window)
        .unwrap()
        .initialize_script("window.x = 42;")
        .add_callback("answer", |_, _, _| {
            println!("hello");
            0
        })
        .load_url("https://tauri.studio")
        .unwrap()
        .build()
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                *control_flow = ControlFlow::Exit
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                webview.resize().unwrap();
            }
            Event::MainEventsCleared => {
                webview.window().request_redraw();
            }
            Event::RedrawRequested(_) => {}
            _ => (),
        }
    });
}
