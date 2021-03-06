use wry::{Application, Attributes, Result, FileDropHandler};

// Apps can have a global file drop handler, and invidiual windows can have their own, too.

static TEST_HTML: &str = r#"data:text/html,
Drop files onto the window and read the console!<br>
Dropping files onto the following form is also possible:<br><br>
<input type="file"/>
"#;

fn main() -> Result<()> {
    #[cfg(not(feature="file-drop"))]
    {
        compile_error!("The file-drop feature needs to be enabled to run this example. e.g. cargo run --example dragndrop --features file-drop")
    }

    let mut app = Application::new()?;

    app.set_file_drop_handler(FileDropHandler::new(|status| {
        println!("Any window: {:?}", status);
        false // Returning true will block the OS default behaviour.
    }));

    app.add_window(Attributes {
        url: Some(TEST_HTML.to_string()),
        file_drop_handler: Some(FileDropHandler::new(|status| {
            println!("Window 1: {:?}", status);
            false // Returning true will block the OS default behaviour.
        })),
        ..Default::default()
    })?;

    app.add_window(Attributes {
        url: Some(TEST_HTML.to_string()),
        file_drop_handler: Some(FileDropHandler::new(|status| {
            println!("Window 2: {:?}", status);
            false // Returning true will block the OS default behaviour.
        })),
        ..Default::default()
    })?;

    app.run();
    Ok(())
}
