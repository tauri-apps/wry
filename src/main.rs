use wry::Result;
use wry::{Application, WebViewAttributes};

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

    let mut app = Application::new()?;
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
