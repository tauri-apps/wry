use wry::Result;
use wry::{Application, Attributes, RpcResponse};
use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
struct MessageParameters {
    message: String,
}

fn main() -> Result<()> {
    let mut app = Application::new()?;

    let html = r#"
<script>
async function getAsyncRpcResult() {
    const reply = await rpc.call('send-parameters', {'message': 'world'});
    console.log(reply);
}
</script>
<div><button onclick="rpc.notify('toggle-fullscreen');">Toggle fullscreen</button></div>
<div><button onclick="getAsyncRpcResult();">Send parameters</button></div>
"#;

    let markup = urlencoding::encode(html);
    let attributes = Attributes {
        url: Some(format!("data:text/html,{}", markup)),
        ..Default::default()
    };

    // NOTE: must be set before calling add_window().
    app.set_rpc_handler(Box::new(|dispatcher, mut req| {
        let mut response = None;
        println!("Rpc handler was called {:?}", req);
        if &req.method == "toggle-fullscreen" {
            println!("Toggle fullscren rpc call...");
        } else if &req.method == "send-parameters" {
            if let Some(params) = req.params.take() {
                if let Some(mut args) = serde_json::from_value::<Vec<MessageParameters>>(params).ok() {
                    let mut result = if args.len() > 0 {
                        let msg = args.swap_remove(0);
                        Some(Value::String(format!("Hello, {}!", msg.message)))
                    } else {
                        // NOTE: in the real-world we should send an error response here!
                        None
                    };
                    // Must always send a response as this is a `call()`
                    response = Some(RpcResponse::new_result(req.id.take(), result));

                    println!("Got rpc parameters {:?}", response);

                }
            }
        }

        //dispatcher.set_fullscreen(true);
        response
    }));

    app.add_window(attributes)?;

    app.run();
    Ok(())
}
