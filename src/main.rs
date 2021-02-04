#[cfg(not(target_os = "linux"))]
use wry::platform::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};
#[cfg(target_os = "linux")]
use wry::platform::{Window, WindowType};
use wry::{Result, WebViewBuilder};

#[cfg(target_os = "linux")]
fn main() -> Result<()> {
    gtk::init().unwrap();
    let window = Window::new(WindowType::Toplevel);

    let webview = WebViewBuilder::new(window)?;
    let w = webview.eval_sender();
    let mut webview = webview
        .init("window.x = 42")?
        .bind("xxx", move |seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            w.send("console.log('The anwser is ' + window.x);").unwrap();
            0
        })?
        .url("https://www.google.com")
        .build()?;

    let w = webview.eval_sender();
    std::thread::spawn(move || {
        w.send("console.log('The anwser is ' + window.x);").unwrap();
    });

    loop {
        webview.dispatch()?;
        gtk::main_iteration();
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> Result<()> {
    let events = EventLoop::new();
    let window = Window::new(&events)?;
    let webview = WebViewBuilder::new(window)?;

    let w = webview.eval_sender();
    let mut webview = webview
        .init("window.x = 42")?
        .bind("xxx", move |seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            w.send("console.log('The anwser is ' + window.x);").unwrap();
            0
        })?
        .url("https://www.google.com")
        .build()?;

    let w = webview.eval_sender();
    std::thread::spawn(move || {
        w.send("console.log('The anwser is ' + window.x);").unwrap();
    });

    events.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        webview.dispatch().unwrap();
        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {}
            _ => (),
        }
    });
}
