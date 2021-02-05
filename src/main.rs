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
    let w = webview.dispatch_sender();
    let mut webview = webview
        .init("window.x = 42")?
        .bind("xxx", move |seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            w.send("console.log('The anwser is ' + window.x);").unwrap();
            0
        })?
        .load_html(
            r#"data:text/html,
            <!doctype html>
            <html>
                <body>hello</body>
                <script>
                    window.onload = function() {
                      document.body.innerText = `hello, ${navigator.userAgent}`;
                    };
                </script>
            </html>"#,
        )?
        .build()?;

    let w = webview.dispatch_sender();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::new(1, 0));
        w.send("console.log('The anwser is ' + window.x);").unwrap();
    });

    loop {
        webview.evaluate()?;
        gtk::main_iteration();
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> Result<()> {
    let events = EventLoop::new();
    let window = Window::new(&events)?;
    let webview = WebViewBuilder::new(window)?;

    let w = webview.dispatch_sender();
    let mut webview = webview
        .init("window.x = 42")?
        .bind("xxx", move |seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            w.send("console.log('The anwser is ' + window.x);").unwrap();
            0
        })?
        .load_html(
            r#"data:text/html,
            <!doctype html>
            <html>
                <body>hello</body>
                <script>
                    window.onload = function() {
                      document.body.innerText = `hello, ${navigator.userAgent}`;
                    };
                </script>
            </html>"#,
        )?
        .build()?;

    let w = webview.dispatch_sender();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::new(1, 0));
        w.send("console.log('The anwser is ' + window.x);").unwrap();
    });

    events.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                webview.resize();
            }
            _ => {
                webview.evaluate().unwrap();
            }
        }
    });
}
