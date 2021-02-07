use wry::Result;
use wry::{Application, Callback, WebViewAttributes};

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
    let callback = Callback {
        name: "xxx".to_owned(),
        function: Box::new(|dispatcher, seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            dispatcher
                .dispatch_script("console.log('The anwser is ' + window.x);")
                .unwrap();
            0
        }),
    };

    let mut app = Application::new()?;
    app.create_window(window1, Some(vec![callback]))?;
    app.create_window(window2, None)?;

    app.run();
    Ok(())
}
