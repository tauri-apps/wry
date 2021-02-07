#[cfg(not(target_os = "linux"))]
mod winit;
#[cfg(not(target_os = "linux"))]
pub use winit::*;
#[cfg(target_os = "linux")]
mod gtkrs;
#[cfg(target_os = "linux")]
pub use gtkrs::*;
