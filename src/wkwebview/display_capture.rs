use crate::wkwebview::class::wry_web_view_ui_delegate::WryWebViewUIDelegate;
use crate::wkwebview::util::operating_system_version;
use crate::WryWebView;
use objc2::{msg_send, DefinedClass, Encode, Encoding};
use objc2_web_kit::WKMediaCaptureType;

#[repr(isize)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WKDisplayCapturePermissionDecision {
    Deny = 0,
    ScreenPrompt,
    WindowPrompt,
}

unsafe impl Encode for WKDisplayCapturePermissionDecision {
    const ENCODING: Encoding = isize::ENCODING;
}

impl From<isize> for WKDisplayCapturePermissionDecision {
    fn from(value: isize) -> Self {
        match value {
            0 => WKDisplayCapturePermissionDecision::Deny,
            1 => WKDisplayCapturePermissionDecision::ScreenPrompt,
            2 => WKDisplayCapturePermissionDecision::WindowPrompt,
            _ => panic!("Invalid WKDisplayCapturePermissionDecision value"),
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn set_decision_handler(
    webview: &WryWebView,
    handler: Option<Box<dyn Fn(WKMediaCaptureType) -> WKDisplayCapturePermissionDecision + 'static>>,
) {
    #[cfg(target_os = "macos")]
    if operating_system_version().0 >= 13 {
        if let Some(handler) = handler {
            let ui_delegate: &WryWebViewUIDelegate = unsafe { msg_send![webview, UIDelegate] };
            ui_delegate.ivars().display_capture_decision_handler.replace(Some(handler));
        }
    }
}
