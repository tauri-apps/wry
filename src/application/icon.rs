// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{error::Error, fmt, io};

use gdk_pixbuf::Pixbuf;

#[derive(Debug)]
/// An error produced when using `Icon::from_rgba` with invalid arguments.
pub enum BadIcon {
  /// Produced when the length of the `rgba` argument isn't divisible by 4, thus `rgba` can't be
  /// safely interpreted as 32bpp RGBA pixels.
  ByteCountNotDivisibleBy4 { byte_count: usize },
  /// Produced when the number of pixels (`rgba.len() / 4`) isn't equal to `width * height`.
  /// At least one of your arguments is incorrect.
  DimensionsVsPixelCount {
    width: u32,
    height: u32,
    width_x_height: usize,
    pixel_count: usize,
  },
  /// Produced when underlying OS functionality failed to create the icon
  OsError(io::Error),
}

impl fmt::Display for BadIcon {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
            BadIcon::ByteCountNotDivisibleBy4 { byte_count } => write!(f,
                "The length of the `rgba` argument ({:?}) isn't divisible by 4, making it impossible to interpret as 32bpp RGBA pixels.",
                byte_count,
            ),
            BadIcon::DimensionsVsPixelCount {
                width,
                height,
                width_x_height,
                pixel_count,
            } => write!(f,
                "The specified dimensions ({:?}x{:?}) don't match the number of pixels supplied by the `rgba` argument ({:?}). For those dimensions, the expected pixel count is {:?}.",
                width, height, pixel_count, width_x_height,
            ),
            BadIcon::OsError(e) => write!(f, "OS error when instantiating the icon: {:?}", e),
        }
  }
}

impl Error for BadIcon {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    Some(self)
  }
}

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
