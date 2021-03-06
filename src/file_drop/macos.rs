use super::{FileDropEvent, FileDropHandler, FileDropListener};

use std::{
	ffi::{c_void, CStr},
	path::PathBuf,
	sync::Once
};

use lazy_static::lazy_static;

use objc::{declare::ClassDecl, runtime::{Object, Sel, Class}};
use cocoa::base::{id, BOOL, YES};

type NSDragOperation = cocoa::foundation::NSUInteger;
#[allow(non_upper_case_globals)]
const NSDragOperationLink: NSDragOperation = 2;

// TODO: don't depend on lazy_static?
// Safety: objc runtime calls are unsafe
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
pub(crate) struct FileDropController {
	listener: *mut FileDropListener
}
impl Drop for FileDropController {
    fn drop(&mut self) {
		// Safety: This could dereference a null ptr.
		// This should never be a null ptr unless something goes wrong in Obj-C.
        unsafe { Box::from_raw(self.listener) };
    }
}
impl FileDropController {
    pub(crate) unsafe fn new(webview: *mut Object, handlers: (Option<FileDropHandler>, Option<FileDropHandler>)) -> Option<FileDropController> {
        if handlers.0.is_none() && handlers.1.is_none() { return None }

        let listener = Box::into_raw(Box::new(FileDropListener::new(handlers)));
        let ptr = listener as *mut FileDropListener;
        (*webview).set_ivar("FileDropListener", ptr as *mut c_void);
        Some(FileDropController {
			listener: ptr
		})
    }

    unsafe fn get_listener(this: &Object) -> &mut FileDropListener {
        let delegate: *mut c_void = *this.get_ivar("FileDropListener");
        &mut *(delegate as *mut FileDropListener)
    }

	unsafe fn collect_paths(drag_info: id) -> Vec<PathBuf> {
        use cocoa::foundation::NSFastEnumeration;
        use cocoa::foundation::NSString;

		let pb: id = msg_send![drag_info, draggingPasteboard];
		let mut file_drop_paths = Vec::new();
		for path in cocoa::appkit::NSPasteboard::propertyListForType(pb, cocoa::appkit::NSFilenamesPboardType).iter() {
			file_drop_paths.push(PathBuf::from(CStr::from_ptr(NSString::UTF8String(path)).to_string_lossy().into_owned()));
		}
		file_drop_paths
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
        let listener = unsafe { FileDropController::get_listener(this) };
        let paths = unsafe { FileDropController::collect_paths(drag_info) };

        if !listener.file_drop(FileDropEvent::Hovered, Some(paths)) {
            // Reject the Wry file drop (invoke the OS default behaviour)
            OBJC_DRAGGING_ENTERED(this, sel, drag_info)
        } else {
            NSDragOperationLink
        }
    }

    extern "C" fn perform_drag_operation(this: &mut Object, sel: Sel, drag_info: id) -> BOOL {
        let listener = unsafe { FileDropController::get_listener(this) };
        let paths = unsafe { FileDropController::collect_paths(drag_info) };

        if !listener.file_drop(FileDropEvent::Dropped, Some(paths)) {
            // Reject the Wry file drop (invoke the OS default behaviour)
            OBJC_PERFORM_DRAG_OPERATION(this, sel, drag_info)
        } else {
            YES
        }
    }

    extern "C" fn dragging_exited(this: &mut Object, sel: Sel, drag_info: id) {
        let listener = unsafe { FileDropController::get_listener(this) };
        if !listener.file_drop(FileDropEvent::Cancelled, None) {
            // Reject the Wry file drop (invoke the OS default behaviour)
            OBJC_DRAGGING_EXITED(this, sel, drag_info);
        }
    }

    // https://github.com/ryanmcgrath/cacao/blob/784727748c60183665cabf3c18fb54896c81214e/src/webview/class.rs#L129
    pub(crate) fn register_webview_class() -> *const Class {
        static mut VIEW_CLASS: *const Class = 0 as *const Class;
        static INIT: Once = Once::new();

        INIT.call_once(|| unsafe {
            let superclass = class!(WKWebView);
            let mut decl = ClassDecl::new("WryWebView", superclass).unwrap();

            decl.add_ivar::<*mut c_void>("FileDropListener");

            decl.add_method(
                sel!(draggingUpdated:),
                FileDropController::dragging_updated as extern "C" fn(&mut Object, Sel, id) -> NSDragOperation,
            );

            decl.add_method(
                sel!(draggingEntered:),
                FileDropController::dragging_entered as extern "C" fn(&mut Object, Sel, id) -> NSDragOperation,
            );

            decl.add_method(
                sel!(performDragOperation:),
                FileDropController::perform_drag_operation as extern "C" fn(&mut Object, Sel, id) -> BOOL,
            );

            decl.add_method(
                sel!(draggingExited:),
                FileDropController::dragging_exited as extern "C" fn(&mut Object, Sel, id),
            );

            VIEW_CLASS = decl.register();
        });

        unsafe { VIEW_CLASS }
    }
}
