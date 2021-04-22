// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use cocoa::{
  appkit::{
    NSButton, NSImage, NSMenu, NSMenuItem, NSSquareStatusItemLength, NSStatusBar, NSStatusItem,
  },
  base::{id, nil, selector},
  foundation::{NSAutoreleasePool, NSData, NSSize, NSString},
};
use objc::{
  declare::ClassDecl,
  runtime::{Object, Sel},
};

extern "C" fn handle_status_bar_menu(_this: &Object, _cmd: Sel, _item: id) {
  // use `handleCustomMenu` as selector to trigger this function
  println!("Custom menu hanlder");
}

// Safety: objc runtime calls are unsafe
pub(crate) unsafe fn add_status_bar_methods(decl: &mut ClassDecl) {
  decl.add_method(
    selector("customMenuClick:"),
    handle_status_bar_menu as extern "C" fn(&Object, _cmd: Sel, item: id),
  );
}

// Safety: objc runtime calls are unsafe
pub(crate) unsafe fn create_status_bar(_webview: *mut Object, title: &str, icon: &[u8]) {
  // todo(lemarier): make it dynamic?
  const ICON_WIDTH: f64 = 18.0;
  const ICON_HEIGHT: f64 = 18.0;

  // create our system status bar
  let status_item = NSStatusBar::systemStatusBar(nil)
    .statusItemWithLength_(NSSquareStatusItemLength)
    .autorelease();

  // set the button title
  let title = NSString::alloc(nil).init_str(title);
  status_item.setTitle_(title);

  let button = status_item.button();
  let menu = NSMenu::new(nil).autorelease();

  // set our icon
  let nsdata = NSData::dataWithBytes_length_(
    nil,
    icon.as_ptr() as *const std::os::raw::c_void,
    icon.len() as u64,
  )
  .autorelease();

  let nsimage = NSImage::initWithData_(NSImage::alloc(nil), nsdata).autorelease();
  let new_size = NSSize::new(ICON_WIDTH, ICON_HEIGHT);

  button.setImage_(nsimage);
  let _: () = msg_send![nsimage, setSize: new_size];

  let no_key = NSString::alloc(nil).init_str("");
  let show_title = NSString::alloc(nil).init_str("Show");
  // todo(lemarier): we should use webview.window().set_visible(false);
  let show_action = selector("unhide:");
  let show_item =
    NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(show_title, show_action, no_key);

  let no_key = NSString::alloc(nil).init_str("");
  let hide_title = NSString::alloc(nil).init_str("Hide");
  // todo(lemarier): we should use webview.window().set_visible(true);
  let hide_action = selector("hide:");
  let hide_item =
    NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(hide_title, hide_action, no_key);

  // more test need to be done, but I think when window is hidden
  // `customMenuClick` is unregistered and make the menu grayed (disabled)
  let no_key = NSString::alloc(nil).init_str("");
  let custom_title = NSString::alloc(nil).init_str("Custom");
  let custom_action = selector("customMenuClick:");
  let custom_item =
    NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(custom_title, custom_action, no_key);

  let no_key = NSString::alloc(nil).init_str("");
  let quit_item = NSString::alloc(nil).init_str("Quit");
  let quit_action = selector("terminate:");
  let quit_item =
    NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(quit_item, quit_action, no_key);

  let separator_first = NSMenuItem::separatorItem(nil).autorelease();

  // build sample menu
  menu.addItem_(show_item);
  menu.addItem_(hide_item);
  menu.addItem_(separator_first);
  menu.addItem_(custom_item);
  menu.addItem_(quit_item);

  status_item.setMenu_(menu);
}
