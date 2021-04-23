// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

pub mod unix {
  use crate::application::window::{Window, WindowRequest};

  pub trait WindowExtUnix {
    fn skip_taskbar(&self);
  }

  impl WindowExtUnix for Window {
    fn skip_taskbar(&self) {
      if let Err(e) = self
        .window_requests_tx
        .send((self.window_id, WindowRequest::SkipTaskbar))
      {
        log::warn!("Fail to send skip taskbar request: {}", e);
      }
    }
  }
}
