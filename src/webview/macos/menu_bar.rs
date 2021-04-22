// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use cocoa::{
  appkit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem},
  base::{id, nil, selector},
  foundation::{NSAutoreleasePool, NSProcessInfo, NSString},
};
use objc::{
  declare::ClassDecl,
  runtime::{Object, Sel},
};

struct KeyEquivalent<'a> {
  key: &'a str,
  masks: Option<NSEventModifierFlags>,
}

// We do not support custom menu right now as we should fix our focus
// first as it prevent a good use of the menu bar
// Related to https://github.com/tauri-apps/wry/issues/184
extern "C" fn handle_menu(_this: &Object, _cmd: Sel, _item: id) {
  // use `handleCustomMenu` as selector to trigger this function
  println!("Custom menu hanlder");
}

// Safety: objc runtime calls are unsafe
pub(crate) unsafe fn add_menu_methods(decl: &mut ClassDecl) {
  decl.add_method(
    selector("handleCustomMenu:"),
    handle_menu as extern "C" fn(&Object, _cmd: Sel, item: id),
  );
  // will be required for our custom menu handler
  // decl.add_ivar::<*mut c_void>("eventProxy");
}

// Safety: objc runtime calls are unsafe
pub(crate) unsafe fn create_menu() {
  /*

    Custom menu implementation:

    we could pass the webview and event loop proxy to create_menu
    the user can then listen to the menu bar event from the UserEvent loop
    of wry

    Something like this could be accomplished, i've done some test and it works:

    let event_loop: EventLoop<Message> = EventLoop::with_user_event();
    event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Wait;

      match event {
          [...]
          Event::UserEvent(event) => {
              println!("{:?}", event);
          }
          _ => (),
      }
    });

    let object: id = msg_send![webview, alloc];
    let () = msg_send![webview, init];
    let proxy: *mut MessageProxy = Box::into_raw(Box::new(menu_bar.event_loop_proxy));
    (*object).set_ivar("eventProxy", proxy as *mut c_void);
  */

  let _pool = NSAutoreleasePool::new(nil);
  let ns_app = NSApplication::sharedApplication(nil);

  // Create menu bar
  let menubar = NSMenu::new(nil).autorelease();
  let app_menu_item = NSMenuItem::new(nil).autorelease();
  let edit_menu_item = NSMenuItem::new(nil).autorelease();

  menubar.addItem_(app_menu_item);
  menubar.addItem_(edit_menu_item);

  // App menu
  let app_menu = NSMenu::new(nil).autorelease();

  // Edit menu
  let edit_menu_title = NSString::alloc(nil).init_str("Edit");
  let edit_menu = NSMenu::alloc(nil)
    .initWithTitle_(edit_menu_title)
    .autorelease();

  // About
  let about_title = NSString::alloc(nil).init_str("About");
  let about_item = menu_item(
    about_title,
    selector("orderFrontStandardAboutPanel:"),
    Some(KeyEquivalent {
      key: "a",
      masks: Some(
        NSEventModifierFlags::NSAlternateKeyMask | NSEventModifierFlags::NSCommandKeyMask,
      ),
    }),
  )
  .autorelease();

  let hide_item_prefix = NSString::alloc(nil).init_str("Hide ");
  let hide_item_title =
    hide_item_prefix.stringByAppendingString_(NSProcessInfo::processInfo(nil).processName());
  let hide_item = menu_item(
    hide_item_title,
    selector("hide:"),
    Some(KeyEquivalent {
      key: "h",
      masks: None,
    }),
  )
  .autorelease();

  // Hide Others
  let show_all_item_title = NSString::alloc(nil).init_str("Show All");
  let show_all_item = menu_item(
    show_all_item_title,
    selector("unhideAllApplications:"),
    None,
  )
  .autorelease();

  // Hide Others
  let hide_others_item_title = NSString::alloc(nil).init_str("Hide Others");
  let hide_others_item = menu_item(
    hide_others_item_title,
    selector("hideOtherApplications:"),
    Some(KeyEquivalent {
      key: "h",
      masks: Some(
        NSEventModifierFlags::NSAlternateKeyMask | NSEventModifierFlags::NSCommandKeyMask,
      ),
    }),
  )
  .autorelease();

  // Quit
  let quit_item_prefix = NSString::alloc(nil).init_str("Quit ");
  let quit_item_title =
    quit_item_prefix.stringByAppendingString_(NSProcessInfo::processInfo(nil).processName());
  let quit_item = menu_item(
    quit_item_title,
    // todo(lemarier): use requestClose hook when available
    // to send notification to all webviews if required
    selector("terminate:"),
    Some(KeyEquivalent {
      key: "q",
      masks: None,
    }),
  )
  .autorelease();

  // Cut
  let cut_title = NSString::alloc(nil).init_str("Cut");
  let cut_item = menu_item(
    cut_title,
    selector("cut:"),
    Some(KeyEquivalent {
      key: "x",
      masks: None,
    }),
  )
  .autorelease();

  // Copy
  let copy_title = NSString::alloc(nil).init_str("Copy");
  let copy_item = menu_item(
    copy_title,
    selector("copy:"),
    Some(KeyEquivalent {
      key: "c",
      masks: None,
    }),
  )
  .autorelease();

  // Paste
  let paste_title = NSString::alloc(nil).init_str("Paste");
  let paste_item = menu_item(
    paste_title,
    selector("paste:"),
    Some(KeyEquivalent {
      key: "v",
      masks: None,
    }),
  )
  .autorelease();

  // Select all
  let select_all_title = NSString::alloc(nil).init_str("Select all");
  let select_all_item = menu_item(
    select_all_title,
    selector("selectAll:"),
    Some(KeyEquivalent {
      key: "a",
      masks: None,
    }),
  )
  .autorelease();

  // our separators for a better style
  let separator_first = NSMenuItem::separatorItem(nil).autorelease();
  let separator_second = NSMenuItem::separatorItem(nil).autorelease();

  // <app name> menu
  // The first menu will always use the app name as name and can't be changed
  // the only way to change it is by modifying the plist and change the CFBundleName
  app_menu.addItem_(about_item);
  app_menu.addItem_(separator_first);
  app_menu.addItem_(hide_item);
  app_menu.addItem_(hide_others_item);
  app_menu.addItem_(show_all_item);
  app_menu.addItem_(separator_second);
  app_menu.addItem_(quit_item);

  // Edit menu
  edit_menu.addItem_(cut_item);
  edit_menu.addItem_(copy_item);
  edit_menu.addItem_(paste_item);
  edit_menu.addItem_(select_all_item);

  // inject our menus
  app_menu_item.setSubmenu_(app_menu);
  edit_menu_item.setSubmenu_(edit_menu);

  ns_app.setMainMenu_(menubar);
}

fn menu_item(
  title: *mut Object,
  selector: Sel,
  key_equivalent: Option<KeyEquivalent<'_>>,
) -> *mut Object {
  unsafe {
    let (key, masks) = match key_equivalent {
      Some(ke) => (NSString::alloc(nil).init_str(ke.key), ke.masks),
      None => (NSString::alloc(nil).init_str(""), None),
    };
    let item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(title, selector, key);
    match masks {
      Some(masks) => item.setKeyEquivalentModifierMask_(masks),
      _ => {}
    }

    item
  }
}
