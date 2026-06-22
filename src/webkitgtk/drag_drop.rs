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

  fn take_paths(&self) -> Option<Vec<PathBuf>> {
    unsafe { &mut *self.paths.get() }.take()
  }

  fn store_position(&self, position: (i32, i32)) {
    self.position.replace(position);
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

  {
    let controller = controller.clone();
    target.connect_enter(move |target, x, y| {
      let position = (x as i32, y as i32);
      controller.store_position(position);
      controller.enter();

      // In GTK4, DropTarget pre-loads data; try to get file list at enter time.
      let paths = extract_paths(target);
      if !paths.is_empty() {
        controller.store_paths(paths.clone());
        controller.call(DragDropEvent::Enter { paths, position });
      }

      gdk::DragAction::COPY
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
        // If Enter was not yet called with paths (value wasn't available then), call it now.
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
      // Only fire Leave if this wasn't triggered by a successful drop.
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
