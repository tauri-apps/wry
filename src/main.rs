use wry::Result;
use wry::{Application, ApplicationDispatcher, Callback, Message, WebViewAttributes};

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
                .dispatch_script("console.log('The anwser is ' + window.x);")
                .unwrap();
            0
        }),
    };

    let mut app = Application::new()?;
    let window1_id = app.create_webview(webview1_attributes, Some(vec![callback]))?;
    app.create_webview(webview2_attributes, None)?;

    app.set_message_handler(|_| {
        println!("got custom message");
    });

    let dispatcher = app.dispatcher();
    let window1_dispatcher = app.window_dispatcher(window1_id);
    let webview1_dispatcher = app.webview_dispatcher(window1_id);

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        dispatcher.dispatch_message(Message::Custom(())).unwrap();
        webview1_dispatcher
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
