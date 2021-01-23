use crate::Error;

use std::ffi::CString;

use cocoa::appkit::{NSApplicationActivationPolicyRegular, NSBackingStoreBuffered};
use core_graphics::geometry::{CGPoint, CGSize, CGRect};
use objc::{
    declare::ClassDecl,
    runtime::{BOOL, Protocol, Object, Sel, YES},
};

type id = *mut Object;

unsafe fn get_nsstring(s: &str) -> id {
    let s = CString::new(s).unwrap();
    let nsstring = class!(NSString);
    msg_send![nsstring, stringWithUTF8String:s.as_ptr()]
}

pub struct InnerWindow {
    window: id,
    webview: bool,
}

// init, navigate, init, eval, bind, run
impl InnerWindow {
    pub fn new() -> Self {
        extern fn yes(_: &Object, _: Sel, _: id) -> BOOL { YES }

        let window = unsafe {
            // Application
            let nsapplication = class!(NSApplication);
            let app: id = msg_send![nsapplication, sharedApplication];
            let _: () = msg_send![app, setActivationPolicy:NSApplicationActivationPolicyRegular];
        
            // Delegate
            let mut cls = ClassDecl::new("AppDelegate", class!(NSResponder)).unwrap();
            cls.add_protocol(Protocol::get("NSTouchBarProvider").unwrap());
            cls.add_method(sel!(applicationShouldTerminateAfterLastWindowClosed:), yes as extern fn(&Object, Sel, id) -> BOOL);
            // TODO bind/on_message/getAssociateObject
            //cls.add_method(sel!(userContentController:didReceiveScriptMessage:), yes as extern fn(&Object, Sel, id) -> BOOL);
            let cls = cls.register();

            let delegate: id = msg_send![cls, new];
            //TODO setAssociateObject
            let _: () = msg_send![app, setDelegate:delegate];

            // Window
            let nswindow = class!(NSWindow);
            let window: id = msg_send![nswindow, alloc];
            let rect = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(800., 600.));
            let window: id = msg_send![window, initWithContentRect:rect styleMask:0 backing:NSBackingStoreBuffered defer:0];

            // Webview
            let wkwebviewconfig = class!(WKWebViewConfiguration);
            let config: id = msg_send![wkwebviewconfig, new];
            let manager: id = msg_send![config, userContentController];
            let wkwebview = class!(WKWebView);
            let webview: id = msg_send![wkwebview, alloc];

            // TODO debug/preference
            
            // TODO init
            
            let _: () = msg_send![webview, initWithFrame:rect configuration:config];
            //let _: () = msg_send![manager, addScriptMessageHandler:delegate name:0];

            let _: () = msg_send![window, setContentView:webview];
            let _: () = msg_send![window, makeKeyAndOrderFront:0];

            let nsurl = class!(NSURL);
            let s = get_nsstring("https://google.com");
            let url: id = msg_send![nsurl, URLWithString:s];
            let nsurlrequest = class!(NSURLRequest);
            let request: id = msg_send![nsurlrequest, requestWithURL:url];
            let _: () = msg_send![webview, loadRequest:request];
            window
        };

        Self {
            window,
            webview: false,
        }
    }

    pub fn run(&self) {
        unsafe {
            let nsapplication = class!(NSApplication);
            let app: id = msg_send![nsapplication, sharedApplication];
            let _: () = msg_send![app, run];
            let _: () = msg_send![app, activateIgmoringOtherApps];
        }
    }
}
