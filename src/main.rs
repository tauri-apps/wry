use wry::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
    Result, WebViewBuilder,
};

fn main() -> Result<()> {
    let events = EventLoop::new();
    let window = Window::new(&events)?;
    let webview = WebViewBuilder::new(window)?;

    let w = webview.eval_handler();
    let webview = webview
        .init("window.x = 42")?
        .bind("xxx", move |seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            w.eval("console.log('The anwser is ' + window.x);").unwrap();
            0
        })?
        .url("https://www.google.com")
        .build()?;

    webview.eval("console.log('The anwser is ' + window.x);")?;
    events.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {}
            _ => (),
        }
    });

    /*
    unsafe {
    let webview = RawWebView::new(true)?;
    RawWebView::init(webview, "window.x = 42")?;
    //RawWebView::eval(webview, "window.x")?;
    RawWebView::bind(webview, "xxx", |_seq, _req| {
        // match webview.eval("console.log('The anwser is ' + window.x);").is_ok() {
        //     true => 0,
        //     false => 1,
        // }
        println!("Hello");
        0
    })?;
    RawWebView::navigate(webview, "https://www.google.com")?;
    RawWebView::run(webview);
    }*/

    // unsafe {
    //     let data = RawWebView::new(true);
    //     RawWebView::set_title(data, "AYAYA")?;
    //     RawWebView::set_size(data, 1024, 768, 0);
    //     RawWebView::init(data, "window.x = 42")?;
    //     RawWebView::bind(
    //         data,
    //         "UwU",
    //         bind,
    //         ptr::null_mut(),
    //     )?;
    //     RawWebView::navigate(
    //         data,
    //         "https://www.google.com/",
    //     )?;
    //     RawWebView::run(data);
    // }
    Ok(())
}

// #[no_mangle]
// extern "C" fn bind(seq: *const c_char, _req: *const c_char, _arg: *mut c_void) -> i32 {
//     unsafe {
//         println!("{}", *seq);
//     }
//     0i32
// }
