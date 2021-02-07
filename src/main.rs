use wry::Result;
use wry::{
    AppWindowAttributes, Application, ApplicationDispatcher, ApplicationExt, Callback, Message,
    WebViewAttributes, WindowExt,
};

fn main() -> Result<()> {
    let webview1_attributes = WebViewAttributes {
        url: Some("https://www.google.com".to_string()),
        initialization_script: vec![String::from("window.x = 42")],
    };
    let webview2_attributes = WebViewAttributes {
        url: Some("https://www.google.com".to_string()),
        initialization_script: vec![String::from("window.x = 24")],
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

    let mut app = Application::<()>::new()?;
    let window1 = app.create_window(AppWindowAttributes::default())?;
    let window1_id = window1.id();
    let window2 = app.create_window(AppWindowAttributes::default())?;
    app.create_webview(window1, webview1_attributes, Some(vec![callback]))?;
    app.create_webview(window2, webview2_attributes, None)?;

    app.set_message_handler(|_| {
        println!("got custom message");
    });

    let dispatcher = app.dispatcher();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        dispatcher.dispatch_message(Message::Custom(())).unwrap();
        dispatcher
            .dispatch_message(Message::Script(
                window1_id,
                "console.log('dispatched message worked')".to_string(),
            ))
            .unwrap();
    });

    app.run();
    Ok(())
}
