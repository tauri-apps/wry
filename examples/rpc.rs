use wry::Result;
use wry::{Application, Attributes};

fn main() -> Result<()> {
    let mut app = Application::new()?;

    let markup = urlencoding::encode(include_str!("rpc.html"));
    let attributes = Attributes {
        url: Some(format!("data:text/html,{}", markup)),
        ..Default::default()
    };

    // NOTE: must be set before calling add_window().
    app.set_rpc_handler(Box::new(|dispatcher, req| {
        println!("Rpc handler was called {:?}", req);
        None 
    }));

    app.add_window(attributes)?;

    app.run();
    Ok(())
}
