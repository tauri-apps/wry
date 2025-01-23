// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::cell::Cell;

use objc2::{
  declare_class, msg_send_id, mutability::MainThreadOnly, rc::Retained, ClassType, DeclaredClass,
};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSEvent, NSView, NSWindow, NSWindowButton};
use objc2_foundation::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_foundation::NSRect;
#[cfg(target_os = "ios")]
use objc2_ui_kit::UIView as NSView;

pub struct WryWebViewParentIvars {
  #[cfg(target_os = "macos")]
  traffic_light_inset: Cell<Option<(f64, f64)>>,
}

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

    #[cfg(target_os = "macos")]
    #[method(drawRect:)]
    fn draw(&self, _dirty_rect: NSRect) {
      if let Some((x, y)) = self.ivars().traffic_light_inset.get() {
        unsafe {inset_traffic_lights(&self.window().unwrap(), x, y)};
      }
    }
  }
);

impl WryWebViewParent {
  #[allow(dead_code)]
  pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
    let delegate = mtm
      .alloc::<WryWebViewParent>()
      .set_ivars(WryWebViewParentIvars {
        #[cfg(target_os = "macos")]
        traffic_light_inset: Default::default(),
      });
    unsafe { msg_send_id![super(delegate), init] }
  }

  #[cfg(target_os = "macos")]
  pub fn set_traffic_light_inset(&self, ns_window: &NSWindow, position: dpi::Position) {
    let scale_factor = NSWindow::backingScaleFactor(ns_window);
    let position = position.to_logical(scale_factor);
    self
      .ivars()
      .traffic_light_inset
      .replace(Some((position.x, position.y)));

    unsafe {
      inset_traffic_lights(ns_window, position.x, position.y);
    }
  }
}

#[cfg(target_os = "macos")]
pub unsafe fn inset_traffic_lights(window: &NSWindow, x: f64, y: f64) {
  let close = window
    .standardWindowButton(NSWindowButton::NSWindowCloseButton)
    .unwrap();
  let miniaturize = window
    .standardWindowButton(NSWindowButton::NSWindowMiniaturizeButton)
    .unwrap();
  let zoom = window
    .standardWindowButton(NSWindowButton::NSWindowZoomButton)
    .unwrap();

  let title_bar_container_view = close.superview().unwrap().superview().unwrap();

  let close_rect = NSView::frame(&close);
  let title_bar_frame_height = close_rect.size.height + y;
  let mut title_bar_rect = NSView::frame(&title_bar_container_view);
  title_bar_rect.size.height = title_bar_frame_height;
  title_bar_rect.origin.y = window.frame().size.height - title_bar_frame_height;
  title_bar_container_view.setFrame(title_bar_rect);

  let space_between = NSView::frame(&miniaturize).origin.x - close_rect.origin.x;
  let window_buttons = vec![close, miniaturize, zoom];

  for (i, button) in window_buttons.into_iter().enumerate() {
    let mut rect = NSView::frame(&button);
    rect.origin.x = x + (i as f64 * space_between);
    button.setFrameOrigin(rect.origin);
  }
}
