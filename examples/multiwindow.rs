use wry::Result;
use wry::{Application, Attributes, Callback};

fn main() -> Result<()> {
    let mut app = Application::new()?;

    let attributes = Attributes {
        url: Some("https://tauri.studio".to_string()),
        // Initialization scripts can be used to define javascript functions and variables.
        initialization_scripts: vec![
            String::from("breads = NaN"),
            String::from("menacing = 'ゴ'"),
        ],
        ..Default::default()
    };
    // Callback defines a rust function to be called on javascript side later. Below is a function
    // which will print the list of parameters after 8th calls.
    let callback = Callback {
        name: "world".to_owned(),
        function: Box::new(|proxy, sequence, requests| {
            // Proxy is like a window handle for you to send message events to the corresponding webview
            // window. You can use it to adjust window and evaluate javascript code like below.
            // This is useful when you want to perform any action in javascript.
            proxy.evaluate_script("console.log(menacing);")?;
            // Sequence is a number counting how many times this function being called.
            if sequence < 8 {
                println!("{} seconds has passed.", sequence);
            } else {
                // Requests is a vector of parameters passed from the caller.
                println!("{:?}", requests);
            }
            Ok(())
        }),
    };

    let window1 = app.add_window(attributes, Some(vec![callback]))?;
    let app_proxy = app.application_proxy();

    std::thread::spawn(move || {
        for _ in 0..7 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            window1.evaluate_script("world()".to_string()).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_secs(1));

        window1.set_title("WRYYYYYYYYYYYYYYYYYYYYY").unwrap();
        let window2 = app_proxy
            .add_window(
                Attributes {
                    width: 426.,
                    height: 197.,
                    title: "RODA RORA DA".into(),
                    url: Some("https://i.imgur.com/x6tXcr9.gif".to_string()),
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        println!("ID of second window: {:?}", window2.id());
    });

    app.run();
    Ok(())
}
