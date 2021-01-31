use wry::*;

fn main() -> Result<()> {
    let webview = WebViewBuilder::new()?;

    let webview = webview
        .init("window.x = 42")?
        .bind("xxx", |seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            0
        })?
        .url("https://www.google.com")
        .build()?;

    webview.eval("console.log('The anwser is ' + window.x);")?;
    webview.run();

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
