use wry::Result;
use wry::{Application, Attributes};

fn main() -> Result<()> {
    let mut app = Application::new()?;

    let markup = urlencoding::encode(include_str!("rpc.html"));
    let attributes = Attributes {
        url: Some(format!("data:text/html,{}", markup)),
        ..Default::default()
    };

    app.add_window(attributes)?;
    app.set_rpc_handler(Box::new(|dispatcher, req| {
        println!("Rpc handler was called {:?}", req);
        None 
    }));
    app.run();
    Ok(())
}
