<img src=".github/splash.png" alt="WRY Webview Rendering library" />

[![](https://img.shields.io/crates/v/wry?style=flat-square)](https://crates.io/crates/wry) [![](https://img.shields.io/docsrs/wry?style=flat-square)](https://docs.rs/wry/)
[![License](https://img.shields.io/badge/License-MIT%20or%20Apache%202-green.svg)](https://opencollective.com/tauri)
[![Chat Server](https://img.shields.io/badge/chat-discord-7289da.svg)](https://discord.gg/SpmNs4S)
[![website](https://img.shields.io/badge/website-tauri.app-purple.svg)](https://tauri.app)
[![https://good-labs.github.io/greater-good-affirmation/assets/images/badge.svg](https://good-labs.github.io/greater-good-affirmation/assets/images/badge.svg)](https://good-labs.github.io/greater-good-affirmation)
[![support](https://img.shields.io/badge/sponsor-Open%20Collective-blue.svg)](https://opencollective.com/tauri)

Cross-platform WebView rendering library in Rust that supports all major desktop platforms like Windows, macOS, and Linux.

<div align="center">
  <a href="https://gfycat.com/needywetelk">
    <img src="https://thumbs.gfycat.com/NeedyWetElk-size_restricted.gif">
  </a>
</div>

## Overview

WRY connects the web engine on each platform and provides easy to use and unified interface to render WebView. It also re-exports [TAO] as a module for event loop and window creation.

[tao]: https://crates.io/crates/tao

## Usage

The minimum example to create a Window and browse a website looks like following:

```rust
fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let _webview = WebViewBuilder::new(window)?
    .with_url("https://tauri.studio")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}
```

There are also more samples under `examples`, you can enter commands like the following to try them:

```
cargo run --example multi_window
```

For more information, please read the documentation below.

## [Documentation](https://docs.rs/wry)

## Platform-specific notes

All platforms use [TAO](https://github.com/tauri-apps/tao) to build the window, and wry re-exports it as an application module. Here is the underlying web engine each platform uses, and some dependencies you might need to install.

### Linux

Tao uses [gtk-rs](https://gtk-rs.org/) and its related libraries for window creation and wry also needs [WebKitGTK](https://webkitgtk.org/) for WebView. So please make sure the following packages are installed:

#### Arch Linux / Manjaro:

```bash
sudo pacman -S webkit2gtk-4.1
```

The `libayatana-indicator` package can be installed from the Arch User Repository (AUR).

#### Debian / Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev
```

#### Fedora

```bash
sudo dnf install gtk3-devel webkit2gtk4.1-devel
```

Fedora does not have the Ayatana package yet, so you need to use the GTK one, see the [feature flags documentation](https://docs.rs/wry/latest/wry/#feature-flags).

### macOS

WebKit is native on macOS so everything should be fine.

If you are cross-compiling for macOS using [osxcross](https://github.com/tpoechtrager/osxcross) and encounter a runtime panic like `Class with name WKWebViewConfiguration could not be found` it's possible that `WebKit.framework` has not been linked correctly, to fix this set the `RUSTFLAGS` environment variable:

```
RUSTFLAGS="-l framework=WebKit" cargo build --target=x86_64-apple-darwin --release
```

### Windows

WebView2 provided by Microsoft Edge Chromium is used. So wry supports Windows 7, 8, 10 and 11.

### Android / iOS

Wry supports mobile with the help of [`cargo-mobile2`](https://github.com/tauri-apps/cargo-mobile2) CLI to create template project. If you are interested in playing or hacking it, please follow [MOBILE.md](MOBILE.md).

If you wish to create Android project yourself, there are a few kotlin files that are needed to run wry on Android and you have to set the following environment variables:

- `WRY_ANDROID_PACKAGE` which is the reversed domain name of your android project and the app name in snake_case for example: `com.wry.example.wry_app`
- `WRY_ANDROID_LIBRARY` for example: if your cargo project has a lib name `wry_app`, it will generate `libwry_app.so` so you se this env var to `wry_app`
- `WRY_ANDROID_KOTLIN_FILES_OUT_DIR` for example: `path/to/app/src/main/kotlin/com/wry/example`

## License

Apache-2.0/MIT
