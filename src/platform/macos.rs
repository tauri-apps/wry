use crate::Result;

use std::{ffi::{c_void, CString}, ptr::null};

use cocoa::appkit::{NSView, NSViewHeightSizable, NSViewWidthSizable};
use cocoa::base::id;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};

use objc::runtime::YES;

unsafe fn get_nsstring(s: &str) -> id {
    let s = CString::new(s).unwrap();
    let nsstring = class!(NSString);
    msg_send![nsstring, stringWithUTF8String:s.as_ptr()]
}

pub struct InnerWebView {
    webview: id,
    manager: id,
}

impl InnerWebView {
    pub fn new(view: *mut c_void) -> Result<Self> {
        unsafe {
            // Webview
            let wkwebviewconfig = class!(WKWebViewConfiguration);
            let config: id = msg_send![wkwebviewconfig, new];
            let manager: id = msg_send![config, userContentController];
            let wkwebview = class!(WKWebView);
            let webview: id = msg_send![wkwebview, alloc];

            // Equivalent Obj-C:
            // [[config preferences] setValue:@YES forKey:@"developerExtrasEnabled"];
            let nsnumber = class!(NSNumber);
            let dev = get_nsstring("developerExtrasEnabled");
            let number: id = msg_send![nsnumber, numberWithBool:1];
            let preference: id = msg_send![config, preferences];
            let _: id = msg_send![preference, setValue:number forKey:dev];
            // TODO webview config

            let rect = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(800., 600.));
            let _: () = msg_send![webview, initWithFrame:rect configuration:config];
            webview.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);

            let view = view as id;
            view.addSubview_(webview);

            Ok(Self { webview, manager })
        }
    }

    pub fn init(&self, js: &str) -> Result<()> {
        // Equivalent Obj-C:
        // [m_manager addUserScript:[[WKUserScript alloc] initWithSource:[NSString stringWithUTF8String:js.c_str()] injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]]
        unsafe {
            let wkuserscript = class!(WKUserScript);
            let userscript: id = msg_send![wkuserscript, alloc];
            let js = get_nsstring(js);
            let script: id = msg_send![userscript, initWithSource:js injectionTime:0 forMainFrameOnly:1];
            let _: () = msg_send![self.manager, addUserScript:script];
        }
        Ok(())
    }

    pub fn eval(&self, js: &str) -> Result<()> {
        unsafe{
            let js = get_nsstring(js);
            let _: id = msg_send![self.webview, evaluateJavaScript:js completionHandler:null::<*const c_void>()];
        }
        Ok(())
    }

    pub fn navigate(&self, url: &str) -> Result<()> {
        unsafe {
            let nsurl = class!(NSURL);
            let s = get_nsstring(url);
            let url: id = msg_send![nsurl, URLWithString: s];
            let nsurlrequest = class!(NSURLRequest);
            let request: id = msg_send![nsurlrequest, requestWithURL: url];
            let _: () = msg_send![self.webview, loadRequest: request];
        }
        Ok(())
    }

    pub fn bind<F>(&self, name: &str, f: F) -> Result<()>
    where
        F: FnMut(i8, Vec<String>) -> i32 + Sync + Send + 'static,
    {
        todo!()
    }
}
