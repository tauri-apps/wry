use wry::{Application, Attributes, Result, FileDropHandler};

// Apps can have a global file drop handler, and invidiual windows can have their own, too.

fn main() -> Result<()> {
    let mut app = Application::new()?;
    
    app.set_file_drop_handler(FileDropHandler::new(|status| {
        println!("Any window: {:?}", status);
        true
    }));

    app.add_window(Attributes {
        url: Some("about:blank".to_string()),
        file_drop_handler: Some(FileDropHandler::new(|status| {
            println!("Window 1: {:?}", status);
            true
        })),
        ..Default::default()
    })?;
    
    app.add_window(Attributes {
        url: Some("about:blank".to_string()),
        file_drop_handler: Some(FileDropHandler::new(|status| {
            println!("Window 2: {:?}", status);
            true
        })),
        ..Default::default()
    })?;

    app.run();
    Ok(())
}
