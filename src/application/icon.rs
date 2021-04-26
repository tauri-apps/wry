// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::io;

use gdk_pixbuf::Pixbuf;
pub use winit::window::BadIcon;

/// An icon used for the window titlebar, taskbar, etc.
#[derive(Debug, Clone)]
pub struct Icon {
  raw: Vec<u8>,
  width: i32,
  height: i32,
  row_stride: i32,
}

impl From<Icon> for Pixbuf {
  fn from(icon: Icon) -> Self {
    Pixbuf::from_mut_slice(
      icon.raw,
      gdk_pixbuf::Colorspace::Rgb,
      true,
      8,
      icon.width,
      icon.height,
      icon.row_stride,
    )
  }
}

impl Icon {
  /// Creates an `Icon` from 32bpp RGBA data.
  ///
  /// The length of `rgba` must be divisible by 4, and `width * height` must equal
  /// `rgba.len() / 4`. Otherwise, this will return a `BadIcon` error.
  pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
    let image = image::load_from_memory(&rgba)
      .map_err(|_| {
        BadIcon::OsError(io::Error::new(
          io::ErrorKind::InvalidData,
          "Invalid icon data!",
        ))
      })?
      .into_rgba8();
    let row_stride = image.sample_layout().height_stride;
    Ok(Icon {
      raw: image.into_raw(),
      width: width as i32,
      height: height as i32,
      row_stride: row_stride as i32,
    })
  }
}
