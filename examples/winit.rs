use wry::{
    platform::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    },
    webview::WebViewBuilder,
    Result,
};

fn main() -> Result<()> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let webview = WebViewBuilder::new(window)
        .unwrap()
        .initialize_script("menacing = 'ゴ';")
        .add_callback("world", |dispatcher, sequence, requests| {
            dispatcher.dispatch_script("console.log(menacing);")?;
            // Sequence is a number counting how many times this function being called.
            if sequence < 8 {
                println!("{} seconds has passed.", sequence);
            } else {
                // Requests is a vector of parameters passed from the caller.
                println!("{:?}", requests);
            }
            Ok(())
        })
        .load_url("https://tauri.studio")?
        .build()?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
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
