use wry::Result;
use wry::{Application, Attributes, WindowProxy, RpcRequest};
use serde_json::Value;

fn main() -> Result<()> {
    let mut app = Application::new()?;

    let html = r#"
<script>    
async function openWindow() {
    await window.rpc.notify("openWindow", "https://i.imgur.com/x6tXcr9.gif");
}
</script>
<p>Multiwindow example</p>
<button onclick="openWindow();">Launch window</button>        
"#;

    let attributes = Attributes {
        url: Some(format!("data:text/html,{}", html)),
        // Initialization scripts can be used to define javascript functions and variables.
        initialization_scripts: vec![
            /* Custom initialization scripts go here */
        ],
        ..Default::default()
    };

    let app_proxy = app.application_proxy();
    let (window_tx, window_rx) = std::sync::mpsc::channel::<String>();

    let handler = Box::new(move |_proxy: &WindowProxy, req: RpcRequest| {
        if &req.method == "openWindow" {
            if let Some(params) = req.params {
                if let Value::Array(mut arr) = params {
                    let mut param = if arr.get(0).is_none() {
                        None
                    } else {
                        Some(arr.swap_remove(0))
                    };

                    if let Some(param) = param.take() {
                        if let Value::String(url) = param {
                            let _ = window_tx.send(url);
                        }
                    }
                }
            }
        }
        None 
    });

    let _ = app.add_window_with_configs(attributes, Some(handler), None)?;

    std::thread::spawn(move || {
        while let Ok(url) = window_rx.recv() {
            let new_window = app_proxy
                .add_window_with_configs(
                    Attributes {
                        width: 426.,
                        height: 197.,
                        title: "RODA RORA DA".into(),
                        url: Some(url),
                        ..Default::default()
                    },
                    None,
                    None,
                )
                .unwrap();
            println!("ID of new window: {:?}", new_window.id());

        }
    });

    app.run();
    Ok(())
}
