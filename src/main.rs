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

use wry::{Application, Callback, WebViewAttributes};

#[cfg(not(target_os = "linux"))]
fn main() -> Result<()> {
    let window1 = WebViewAttributes {
        url: Some("https://www.google.com".to_string()),
        initialization_script: vec![String::from("window.x = 42")],
        bind: vec![Callback {
            name: "xxx".to_string(),
            function: Box::new(|seq, req| {
                println!("The seq is: {}", seq);
                println!("The req is: {:?}", req);
                0
            }),
            evaluation_script: Some("console.log('The anwser is ' + window.x);".to_string()),
        }],
        ..Default::default()
    };
    let window2 = WebViewAttributes {
        title: "window 2".to_string(),
        url: Some("https://www.google.com".to_string()),
        initialization_script: vec![String::from("window.x = 24")],
        bind: vec![Callback {
            name: "xxx".to_string(),
            function: Box::new(|seq, req| {
                println!("The seq is: {}", seq);
                println!("The req is: {:?}", req);
                0
            }),
            evaluation_script: Some("console.log('The anwser is ' + window.x);".to_string()),
        }],
        ..Default::default()
    };
    let mut app = Application::new();
    app.add_webview(window1)?;
    app.add_webview(window2)?;
    app.run();
    Ok(())
}
