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

use wry::{Application, WebViewAttributes};

#[cfg(not(target_os = "linux"))]
fn main() -> Result<()> {
    let window1 = WebViewAttributes {
        url: Some("https://www.google.com".to_string()),
        initialization_script: vec![String::from("window.x = 42")],
        ..Default::default()
    };
    let window2 = WebViewAttributes {
        title: "window 2".to_string(),
        initialization_script: vec![String::from("window.x = 24")],
        ..window1.clone()
    };

    let mut app = Application::new();
    let mut webview1 = app.create_webview(window1)?;
    let w = webview1.dispatcher();
    webview1 = webview1.add_callback("xxx", move |seq, req| {
        println!("The seq is: {}", seq);
        println!("The req is: {:?}", req);
        w.send("console.log('The anwser is ' + window.x);").unwrap();
        0
    })?;
    let webview2 = app.create_webview(window2)?;

    app.add_webview(webview1.build()?);
    app.add_webview(webview2.build()?);
    app.run();
    Ok(())
}
