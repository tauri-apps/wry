use wry::platform::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    platform::windows::WindowExtWindows,
    winapi::shared::windef::RECT,
    win::*
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut webview = InnerWebView::new(window.hwnd()).unwrap();
    webview.init("window.x = 42;");
    webview.navigate("https://tauri.studio").unwrap();
    webview.build();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                *control_flow = ControlFlow::Exit
            },
            Event::WindowEvent {
                event: WindowEvent::Resized(new_size),
                ..
            } => {
                if let Some(webview_host) = webview.controller.get() {
                    let r = RECT {
                        left: 0,
                        top: 0,
                        right: new_size.width as i32,
                        bottom: new_size.height as i32,
                    };
                    webview_host.put_bounds(r).expect("put_bounds");
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            },
            Event::RedrawRequested(_) => {
            },
            _ => ()
        }
        //webview.eval("console.log(window.x);");
    });
}