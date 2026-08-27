// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  cell::{Cell, UnsafeCell},
  path::PathBuf,
  rc::Rc,
};

use webkit6::gdk;
use webkit6::glib;
use webkit6::gtk;
use webkit6::prelude::*;
use webkit6::WebView;

use crate::DragDropEvent;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
enum DragControllerState {
  Entered,
  Dropped,
  Left,
}

struct DragDropController {
  paths: UnsafeCell<Option<Vec<PathBuf>>>,
  state: Cell<DragControllerState>,
  position: Cell<(i32, i32)>,
  handler: Box<dyn Fn(DragDropEvent) -> bool>,
}

impl DragDropController {
  fn new(handler: Box<dyn Fn(DragDropEvent) -> bool>) -> Self {
    Self {
      handler,
      paths: UnsafeCell::new(None),
      state: Cell::new(DragControllerState::Left),
      position: Cell::new((0, 0)),
    }
  }

  fn store_paths(&self, paths: Vec<PathBuf>) {
    unsafe { *self.paths.get() = Some(paths) };
  }

  fn has_paths(&self) -> bool {
    unsafe { (*self.paths.get()).is_some() }
  }

  fn take_paths(&self) -> Option<Vec<PathBuf>> {
    unsafe { &mut *self.paths.get() }.take()
  }

  // Clear stale paths from a previous drag session at the start of a new one.
  fn reset_paths(&self) {
    unsafe { *self.paths.get() = None };
  }

  fn store_position(&self, position: (i32, i32)) {
    self.position.replace(position);
  }

  fn position(&self) -> (i32, i32) {
    self.position.get()
  }

  fn enter(&self) {
    self.state.set(DragControllerState::Entered);
  }

  fn dropped(&self) {
    self.state.set(DragControllerState::Dropped);
  }

  fn leave(&self) {
    self.state.set(DragControllerState::Left);
  }

  fn state(&self) -> DragControllerState {
    self.state.get()
  }

  fn call(&self, event: DragDropEvent) -> bool {
    (self.handler)(event)
  }
}

pub(crate) fn connect_drag_event(webview: &WebView, handler: Box<dyn Fn(DragDropEvent) -> bool>) {
  let controller = Rc::new(DragDropController::new(handler));

  let target = gtk::DropTarget::new(
    gdk::FileList::static_type(),
    gdk::DragAction::COPY | gdk::DragAction::MOVE,
  );

  // Instruct GTK to fetch the drag payload while the pointer is hovering.
  // Without this, GDK defers the wl_data_device transfer until the drop signal,
  // so file paths are never available in connect_enter on Wayland — matching the
  // synchronous behaviour of the macOS pasteboard and Win32 IDataObject APIs.
  target.set_preload(true);

  {
    let controller = controller.clone();
    target.connect_enter(move |target, x, y| {
      let position = (x as i32, y as i32);
      // Reset stale paths from a previous drag that ended without a drop.
      controller.reset_paths();
      controller.store_position(position);
      controller.enter();

      // On X11 (and sometimes Wayland when data is already cached) the file
      // list is available immediately; fire Enter now so the sequence matches
      // macOS/Windows. On Wayland the notify::value handler covers the async case.
      let paths = extract_paths(target);
      if !paths.is_empty() {
        controller.store_paths(paths.clone());
        controller.call(DragDropEvent::Enter { paths, position });
      }

      gdk::DragAction::COPY
    });
  }

  {
    // Fires once GDK has finished fetching the preloaded drag value — the primary
    // path for Wayland where wl_data_device transfer is asynchronous.
    let controller = controller.clone();
    target.connect_notify_local(Some("value"), move |target, _| {
      if controller.state() == DragControllerState::Entered && !controller.has_paths() {
        let paths = extract_paths(target);
        if !paths.is_empty() {
          let position = controller.position();
          controller.store_paths(paths.clone());
          controller.call(DragDropEvent::Enter { paths, position });
        }
      }
    });
  }

  {
    let controller = controller.clone();
    target.connect_motion(move |_, x, y| {
      let position = (x as i32, y as i32);
      controller.store_position(position);
      if controller.state() == DragControllerState::Entered {
        controller.call(DragDropEvent::Over { position });
      }
      gdk::DragAction::COPY
    });
  }

  {
    let controller = controller.clone();
    target.connect_drop(move |_, value, x, y| {
      let position = (x as i32, y as i32);
      if let Some(paths) = value
        .get::<gdk::FileList>()
        .ok()
        .map(|fl| paths_from_file_list(&fl))
      {
        // Safety net: if Enter was never fired (preload unavailable), emit it now
        // immediately before Drop so the event order contract is always honoured.
        if controller.take_paths().is_none() {
          controller.call(DragDropEvent::Enter {
            paths: paths.clone(),
            position,
          });
        }

        controller.dropped();
        controller.call(DragDropEvent::Drop { paths, position })
      } else {
        false
      }
    });
  }

  target.connect_leave(move |_| {
    if controller.state() != DragControllerState::Left {
      // Only fire Leave when the pointer exited without a drop; a successful
      // drop transitions state to Dropped before leave fires.
      let should_fire_leave = controller.state() == DragControllerState::Entered;
      controller.leave();
      if should_fire_leave {
        let controller = controller.clone();
        glib::idle_add_local_once(move || {
          controller.call(DragDropEvent::Leave);
        });
      }
    }
  });

  webview.add_controller(target);
}

pub(crate) fn connect_drag_source(
  webview: &WebView,
  handler: Box<dyn Fn(i32, i32) -> Option<String>>,
) {
  let source = gtk::DragSource::new();
  source.connect_prepare(move |_, x, y| {
    handler(x as i32, y as i32).map(|text| gdk::ContentProvider::for_value(&text.to_value()))
  });
  webview.add_controller(source);
}

fn extract_paths(target: &gtk::DropTarget) -> Vec<PathBuf> {
  target
    .value()
    .and_then(|v| v.get::<gdk::FileList>().ok())
    .map(|fl| paths_from_file_list(&fl))
    .unwrap_or_default()
}

fn paths_from_file_list(file_list: &gdk::FileList) -> Vec<PathBuf> {
  file_list
    .files()
    .into_iter()
    .filter_map(|f| f.path())
    .collect()
}
