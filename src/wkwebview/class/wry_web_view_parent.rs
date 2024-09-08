// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use objc2::{
  declare_class, msg_send_id, mutability::MainThreadOnly, rc::Retained, ClassType, DeclaredClass,
};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSEvent, NSView};
use objc2_foundation::MainThreadMarker;
#[cfg(target_os = "ios")]
use objc2_ui_kit::UIView as NSView;

pub struct WryWebViewParentIvars {}

declare_class!(
  pub struct WryWebViewParent;

  unsafe impl ClassType for WryWebViewParent {
    type Super = NSView;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryWebViewParent";
  }

  impl DeclaredClass for WryWebViewParent {
    type Ivars = WryWebViewParentIvars;
  }

  unsafe impl WryWebViewParent {
    #[cfg(target_os = "macos")]
    #[method(keyDown:)]
    fn key_down(
      &self,
      event: &NSEvent,
    ) {
      let mtm = MainThreadMarker::new().unwrap();
      let app = NSApplication::sharedApplication(mtm);
      unsafe {
        if let Some(menu) = app.mainMenu() {
          menu.performKeyEquivalent(event);
        }
      }
    }
  }
);

impl WryWebViewParent {
  #[allow(dead_code)]
  pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
    let delegate = mtm
      .alloc::<WryWebViewParent>()
      .set_ivars(WryWebViewParentIvars {});
    unsafe { msg_send_id![super(delegate), init] }
  }
}
