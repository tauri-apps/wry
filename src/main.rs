use wry::Result;
use wry::{Application, Callback, WebViewAttributes};

fn main() -> Result<()> {
    let webview1_attributes = WebViewAttributes {
        url: Some("https://www.google.com".to_string()),
        initialization_scripts: vec![String::from("window.x = 42")],
        ..Default::default()
    };
    let webview2_attributes = WebViewAttributes {
        url: Some("https://www.google.com".to_string()),
        initialization_scripts: vec![String::from("window.x = 24")],
        ..Default::default()
    };
    let callback = Callback {
        name: "xxx".to_owned(),
        function: Box::new(|dispatcher, seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            dispatcher
                .eval_script("console.log('The anwser is ' + window.x);")
                .unwrap();
            0
        }),
    };

    let mut app = Application::new()?;
    let window1_dispatcher = app.add_window(webview1_attributes, Some(vec![callback]))?;
    app.add_window(webview2_attributes, None)?;

    let dispatcher = app.application_proxy();

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        window1_dispatcher
            .eval_script("console.log('dispatched message worked')".to_string())
            .unwrap();

        window1_dispatcher.set_title("new title").unwrap();

        let window_id = dispatcher
            .add_window(
                WebViewAttributes {
                    title: "NEW WINDOW".into(),
                    url: Some("https://www.google.com".to_string()),
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        println!("{:?}", window_id);
    });

    app.run();
    Ok(())
}
