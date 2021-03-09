use wry::{Application, Attributes, Result};

// Apps can have a global file drop handler, and invidiual windows can have their own, too.

static TEST_HTML: &str = r#"data:text/html,
Drop files onto the window and read the console!<br>
Dropping files onto the following form is also possible:<br><br>
<input type="file"/>
"#;

fn main() -> Result<()> {
    #[cfg(not(feature = "file-drop"))]
    {
        compile_error!("The file-drop feature needs to be enabled to run this example. e.g. cargo run --example dragndrop --features file-drop")
    }

    let mut app = Application::new()?;

    app.add_window_with_configs(
        Attributes {
            url: Some(TEST_HTML.to_string()),
            ..Default::default()
        },
        Some(Box::new(|data| {
            println!("Window 1: {:?}", data);
            false // Returning true will block the OS default behaviour.
        })),
        None,
        None,
    )?;
    app.run();
    Ok(())
}
