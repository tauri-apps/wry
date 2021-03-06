use crate::mimetype::MimeType;
use crate::application::{WindowProxy, FileDropHandler, FileDropEvent, FileDropController};
use crate::webview::WV;
use crate::{Result, RpcHandler};

use std::{
    ffi::{c_void, CStr},
    os::raw::c_char,
    ptr::null,
    slice, str,
    sync::Once,
    path::PathBuf,
};

use cocoa::appkit::{NSView, NSViewHeightSizable, NSViewWidthSizable};
use cocoa::base::{id, BOOL, YES};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use lazy_static::lazy_static;
use objc::{declare::ClassDecl, runtime::{Object, Sel, Class}};
use objc_id::Id;
use url::Url;
use winit::{platform::macos::WindowExtMacOS, window::Window};

type NSDragOperation = cocoa::foundation::NSUInteger;
#[allow(non_upper_case_globals)]
const NSDragOperationLink: NSDragOperation = 2;

// TODO: don't depend on lazy_static?
use objc::runtime::class_getInstanceMethod;
use objc::runtime::method_getImplementation;
lazy_static! {
    static ref OBJC_DRAGGING_ENTERED: extern "C" fn(*const Object, Sel, id) -> NSDragOperation = unsafe {
        std::mem::transmute(
            method_getImplementation(class_getInstanceMethod(class!(WKWebView), sel!(draggingEntered:)))
        )
    };
    static ref OBJC_DRAGGING_EXITED: extern "C" fn(*const Object, Sel, id) = unsafe {
        std::mem::transmute(
            method_getImplementation(class_getInstanceMethod(class!(WKWebView), sel!(draggingExited:)))
        )
    };
    static ref OBJC_PERFORM_DRAG_OPERATION: extern "C" fn(*const Object, Sel, id) -> BOOL = unsafe {
        std::mem::transmute(
            method_getImplementation(class_getInstanceMethod(class!(WKWebView), sel!(performDragOperation:)))
        )
    };
    static ref OBJC_DRAGGING_UPDATED: extern "C" fn(*const Object, Sel, id) -> NSDragOperation = unsafe {
        std::mem::transmute(
            method_getImplementation(class_getInstanceMethod(class!(WKWebView), sel!(draggingUpdated:)))
        )
    };
}

// This struct contains functions which will be "injected" into the WKWebView,
// + any relevant helper functions.
// Safety: objc runtime calls are unsafe
struct FileDropDelegate;
impl FileDropDelegate {
    unsafe fn init(webview: *mut Object, handlers: (Option<FileDropHandler>, Option<FileDropHandler>)) -> Option<*mut FileDropController> {
        if handlers.0.is_none() && handlers.1.is_none() { return None }

        let controller = Box::leak(Box::new(FileDropController::new(handlers)));
        let ptr = controller as *mut FileDropController;
        (*webview).set_ivar("FileDropController", ptr as *mut c_void);
        Some(ptr)
    }

    unsafe fn get_controller(this: &Object) -> &mut FileDropController {
        let delegate: *mut c_void = *this.get_ivar("FileDropController");
        &mut *(delegate as *mut FileDropController)
    }

    extern "C" fn dragging_updated(this: &mut Object, sel: Sel, drag_info: id) -> NSDragOperation {
        let os_operation = OBJC_DRAGGING_UPDATED(this, sel, drag_info);
        if os_operation == 0 {
            // 0 will be returned for a file drop on any arbitrary location on the webview.
            // We'll override that with NSDragOperationLink.
            NSDragOperationLink
        } else {
            // A different NSDragOperation is returned when a file is hovered over something like
            // a <input type="file">, so we'll make sure to preserve that behaviour.
            os_operation
        }
    }
    
    extern "C" fn dragging_entered(this: &mut Object, sel: Sel, drag_info: id) -> NSDragOperation {
        use cocoa::foundation::NSFastEnumeration;
        use cocoa::foundation::NSString;

        let controller = unsafe { FileDropDelegate::get_controller(this) };

        let paths = unsafe {
            let pb: id = msg_send![drag_info, draggingPasteboard];
            let mut file_drop_paths = Vec::new();
            for path in cocoa::appkit::NSPasteboard::propertyListForType(pb, cocoa::appkit::NSFilenamesPboardType).iter() {
                file_drop_paths.push(PathBuf::from(CStr::from_ptr(NSString::UTF8String(path)).to_string_lossy().into_owned()));
            }
            file_drop_paths
        };

        if !controller.file_drop(FileDropEvent::Hovered, Some(paths)) {
            // Reject the Wry file drop (invoke the OS default behaviour)
            OBJC_DRAGGING_ENTERED(this, sel, drag_info)
        } else {
            NSDragOperationLink
        }
    }

    extern "C" fn perform_drag_operation(this: &mut Object, sel: Sel, drag_info: id) -> BOOL {
        use cocoa::foundation::NSFastEnumeration;
        use cocoa::foundation::NSString;

        let controller = unsafe { FileDropDelegate::get_controller(this) };

        let paths = unsafe {
            let pb: id = msg_send![drag_info, draggingPasteboard];
            let mut file_drop_paths = Vec::new();
            for path in cocoa::appkit::NSPasteboard::propertyListForType(pb, cocoa::appkit::NSFilenamesPboardType).iter() {
                file_drop_paths.push(PathBuf::from(CStr::from_ptr(NSString::UTF8String(path)).to_string_lossy().into_owned()));
            }
            file_drop_paths
        };

        if !controller.file_drop(FileDropEvent::Dropped, Some(paths)) {
            // Reject the Wry file drop (invoke the OS default behaviour)
            OBJC_PERFORM_DRAG_OPERATION(this, sel, drag_info)
        } else {
            YES
        }
    }

    extern "C" fn dragging_exited(this: &mut Object, sel: Sel, drag_info: id) {
        let controller = unsafe { FileDropDelegate::get_controller(this) };
        if !controller.file_drop(FileDropEvent::Cancelled, None) {
            // Reject the Wry file drop (invoke the OS default behaviour)
            OBJC_DRAGGING_EXITED(this, sel, drag_info);
        }
    }

    // https://github.com/ryanmcgrath/cacao/blob/784727748c60183665cabf3c18fb54896c81214e/src/webview/class.rs#L129
    fn register_webview_class() -> *const Class {
        static mut VIEW_CLASS: *const Class = 0 as *const Class;
        static INIT: Once = Once::new();

        INIT.call_once(|| unsafe {
            let superclass = class!(WKWebView);
            let mut decl = ClassDecl::new("WryWebView", superclass).unwrap();
            
            decl.add_ivar::<*mut c_void>("FileDropController");

            decl.add_method(
                sel!(draggingUpdated:),
                FileDropDelegate::dragging_updated as extern "C" fn(&mut Object, Sel, id) -> NSDragOperation,
            );

            decl.add_method(
                sel!(draggingEntered:),
                FileDropDelegate::dragging_entered as extern "C" fn(&mut Object, Sel, id) -> NSDragOperation,
            );

            decl.add_method(
                sel!(performDragOperation:),
                FileDropDelegate::perform_drag_operation as extern "C" fn(&mut Object, Sel, id) -> BOOL,
            );

            decl.add_method(
                sel!(draggingExited:),
                FileDropDelegate::dragging_exited as extern "C" fn(&mut Object, Sel, id),
            );

            VIEW_CLASS = decl.register();
        });

        unsafe { VIEW_CLASS }
    }
}

pub struct InnerWebView {
    webview: Id<Object>,
    manager: id,
    file_drop_controller: Option<*mut FileDropController>
}

impl WV for InnerWebView {
    type Window = Window;

    fn new<F: 'static + Fn(&str) -> Result<Vec<u8>>>(

        window: &Window,
        scripts: Vec<String>,
        url: Option<Url>,
        transparent: bool,
        custom_protocol: Option<(String, F)>,
        rpc_handler: Option<RpcHandler>,
        file_drop_handlers: (Option<FileDropHandler>, Option<FileDropHandler>),

    ) -> Result<Self> {
        // Callback function for message handler
        extern "C" fn did_receive(this: &Object, _: Sel, _: id, msg: id) {
            // Safety: objc runtime calls are unsafe
            unsafe {
                let function = this.get_ivar::<*mut c_void>("function");
                let function: &mut RpcHandler = std::mem::transmute(*function);
                let body: id = msg_send![msg, body];
                let utf8: *const c_char = msg_send![body, UTF8String];
                let js = CStr::from_ptr(utf8).to_str().expect("Invalid UTF8 string");

                match super::rpc_proxy(js.to_string(), function) {
                    Ok(result) => {
                        if let Some(ref script) = result {
                            let wv: id = msg_send![msg, webView];
                            let js = NSString::new(script);
                            let _: id = msg_send![wv, evaluateJavaScript:js completionHandler:null::<*const c_void>()];
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
        }

        // Task handler for custom protocol
        extern "C" fn start_task(this: &Object, _: Sel, _webview: id, task: id) {
            unsafe {
                let function = this.get_ivar::<*mut c_void>("function");
                let function: &mut Box<dyn Fn(&str) -> Result<Vec<u8>>> =
                    std::mem::transmute(*function);

                // Get url request
                let request: id = msg_send![task, request];
                let url: id = msg_send![request, URL];
                let nsstring = {
                    let s: id = msg_send![url, absoluteString];
                    NSString(Id::from_ptr(s))
                };
                let uri = nsstring.to_str();

                // Send response
                if let Ok(content) = function(uri) {
                    let mime = MimeType::parse(&content, uri);
                    let nsurlresponse: id = msg_send![class!(NSURLResponse), alloc];
                    let response: id = msg_send![nsurlresponse, initWithURL:url MIMEType:NSString::new(&mime)
                        expectedContentLength:content.len() textEncodingName:null::<c_void>()];
                    let () = msg_send![task, didReceiveResponse: response];

                    // Send data
                    let bytes = content.as_ptr() as *mut c_void;
                    let data: id = msg_send![class!(NSData), alloc];
                    let data: id = msg_send![data, initWithBytes:bytes length:content.len()];
                    let () = msg_send![task, didReceiveData: data];

                    // Finish
                    let () = msg_send![task, didFinish];
                }
            }
        }
        extern "C" fn stop_task(_: &Object, _: Sel, _webview: id, _task: id) {}

        // Safety: objc runtime calls are unsafe
        unsafe {
            // Config and custom protocol
            let config: id = msg_send![class!(WKWebViewConfiguration), new];
            if let Some((name, function)) = custom_protocol {
                let cls = ClassDecl::new("CustomURLSchemeHandler", class!(NSObject));
                let cls = match cls {
                    Some(mut cls) => {
                        cls.add_ivar::<*mut c_void>("function");
                        cls.add_method(
                            sel!(webView:startURLSchemeTask:),
                            start_task as extern "C" fn(&Object, Sel, id, id),
                        );
                        cls.add_method(
                            sel!(webView:stopURLSchemeTask:),
                            stop_task as extern "C" fn(&Object, Sel, id, id),
                        );
                        cls.register()
                    }
                    None => class!(CustomURLSchemeHandler),
                };
                let handler: id = msg_send![cls, new];
                let function: Box<Box<dyn Fn(&str) -> Result<Vec<u8>>>> =
                    Box::new(Box::new(function));

                (*handler).set_ivar("function", Box::into_raw(function) as *mut _ as *mut c_void);
                let () = msg_send![config, setURLSchemeHandler:handler forURLScheme:NSString::new(&name)];
            }

            // First, since we may have to use a custom class to hook into file drop operations,
            // we need to define WHICH class we're going to use to create the WKWebView.
            // If there are no file drop handlers set, we can just alloc a new WKWebView.
            // Otherwise, we'll alloc a WryWebView.

            let use_custom_webview_class = file_drop_handlers.0.is_some() || file_drop_handlers.1.is_some();

            let webview_class = match use_custom_webview_class {
                true => FileDropDelegate::register_webview_class(),
                false => class!(WKWebView)
            };

            // Webview and manager
            let manager: id = msg_send![config, userContentController];
            let webview: id = msg_send![webview_class, alloc];
            let preference: id = msg_send![config, preferences];
            let yes: id = msg_send![class!(NSNumber), numberWithBool:1];
            let no: id = msg_send![class!(NSNumber), numberWithBool:0];

            debug_assert_eq!(
                {
                    // Equivalent Obj-C:
                    // [[config preferences] setValue:@YES forKey:@"developerExtrasEnabled"];
                    let dev = NSString::new("developerExtrasEnabled");
                    let _: id = msg_send![preference, setValue:yes forKey:dev];
                },
                ()
            );

            if transparent {
                // Equivalent Obj-C:
                // [config setValue:@NO forKey:@"drawsBackground"];
                let _: id = msg_send![config, setValue:no forKey:NSString::new("drawsBackground")];
            }

            // Resize
            let size = window.inner_size().to_logical(window.scale_factor());
            let rect = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(size.width, size.height));
            let _: () = msg_send![webview, initWithFrame:rect configuration:config];
            webview.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);

            // Message handler
            if let Some(rpc_handler) = rpc_handler {
                let cls = ClassDecl::new("WebViewDelegate", class!(NSObject));
                let cls = match cls {
                    Some(mut cls) => {
                        cls.add_ivar::<*mut c_void>("function");
                        cls.add_method(
                            sel!(userContentController:didReceiveScriptMessage:),
                            did_receive as extern "C" fn(&Object, Sel, id, id),
                        );
                        cls.register()
                    }
                    None => class!(WebViewDelegate),
                };
                let handler: id = msg_send![cls, new];
                let function: Box<RpcHandler> = Box::new(rpc_handler);

                (*handler).set_ivar("function", Box::into_raw(function) as *mut _ as *mut c_void);
                let external = NSString::new("external");
                let _: () = msg_send![manager, addScriptMessageHandler:handler name:external];
            }

            // File drop handling
            let file_drop_controller = FileDropDelegate::init(webview, file_drop_handlers);

            let w = Self {
                webview: Id::from_ptr(webview),
                manager,
                file_drop_controller,
            };

            // Initialize scripts
            w.init(
                r#"window.external = {
                    invoke: function(s) {
                        window.webkit.messageHandlers.external.postMessage(s);
                    },
                };

                window.addEventListener("keydown", function(e) {
                    if (e.defaultPrevented) {
                        return;
                    }

                   if (e.metaKey) {
                        switch(e.key) {
                            case "x":
                                document.execCommand("cut");
                                e.preventDefault();
                                break;
                            case "c":
                                document.execCommand("copy");
                                e.preventDefault();
                                break;
                            case "v":
                                document.execCommand("paste");
                                e.preventDefault();
                                break;
                            default:
                                return;
                        }
                    }
                }, true);"#,
            );
            for js in scripts {
                w.init(&js);
            }

            // Navigation
            if let Some(url) = url {
                if url.cannot_be_a_base() {
                    let s = url.as_str();
                    if let Some(pos) = s.find(',') {
                        let (_, path) = s.split_at(pos + 1);
                        w.navigate_to_string(path);
                    }
                } else {
                    w.navigate(url.as_str());
                }
            }

            let view = window.ns_view() as id;
            view.addSubview_(webview);

            Ok(w)
        }
    }

    fn eval(&self, js: &str) -> Result<()> {
        // Safety: objc runtime calls are unsafe
        unsafe {
            let _: id = msg_send![self.webview, evaluateJavaScript:NSString::new(js) completionHandler:null::<*const c_void>()];
        }
        Ok(())
    }
}

impl InnerWebView {
    fn init(&self, js: &str) {
        // Safety: objc runtime calls are unsafe
        // Equivalent Obj-C:
        // [manager addUserScript:[[WKUserScript alloc] initWithSource:[NSString stringWithUTF8String:js.c_str()] injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]]
        unsafe {
            let userscript: id = msg_send![class!(WKUserScript), alloc];
            let script: id = msg_send![userscript, initWithSource:NSString::new(js) injectionTime:0 forMainFrameOnly:1];
            let _: () = msg_send![self.manager, addUserScript: script];
        }
    }

    fn navigate(&self, url: &str) {
        // Safety: objc runtime calls are unsafe
        unsafe {
            let url: id = msg_send![class!(NSURL), URLWithString: NSString::new(url)];
            let request: id = msg_send![class!(NSURLRequest), requestWithURL: url];
            let () = msg_send![self.webview, loadRequest: request];
        }
    }

    fn navigate_to_string(&self, url: &str) {
        // Safety: objc runtime calls are unsafe
        unsafe {
            let empty: id = msg_send![class!(NSURL), URLWithString: NSString::new("")];
            let () = msg_send![self.webview, loadHTMLString:NSString::new(url) baseURL:empty];
        }
    }
}

impl Drop for InnerWebView {
    fn drop(&mut self) {
        if let Some(ptr) = self.file_drop_controller {
            // Safety: It's possible for this pointer to be a nullptr which could cause a panic.
            // This ptr should never be null as it has been leaked by Box::leak. It could only be dropped
            // by the Objective-C side which does not manage memory like Rust and therefore will not drop it unless
            // explicitly instructed to.
            unsafe { std::ptr::drop_in_place(ptr) };
        }
    }
}

const UTF8_ENCODING: usize = 4;

struct NSString(Id<Object>);

impl NSString {
    fn new(s: &str) -> Self {
        // Safety: objc runtime calls are unsafe
        NSString(unsafe {
            let nsstring: id = msg_send![class!(NSString), alloc];
            Id::from_ptr(
                msg_send![nsstring, initWithBytes:s.as_ptr() length:s.len() encoding:UTF8_ENCODING],
            )
        })
    }

    fn to_str(&self) -> &str {
        unsafe {
            let bytes: *const c_char = msg_send![self.0, UTF8String];
            let len = msg_send![self.0, lengthOfBytesUsingEncoding: UTF8_ENCODING];
            let bytes = slice::from_raw_parts(bytes as *const u8, len);
            str::from_utf8(bytes).unwrap()
        }
    }
}
