<img src=".github/splash.png" alt="WRY Webview Rendering library" />

[![](https://img.shields.io/crates/v/wry?style=flat-square)](https://crates.io/crates/wry) [![](https://img.shields.io/docsrs/wry?style=flat-square)](https://docs.rs/wry/)
[![License](https://img.shields.io/badge/License-MIT%20or%20Apache%202-green.svg)](https://opencollective.com/tauri)
[![Chat Server](https://img.shields.io/badge/chat-discord-7289da.svg)](https://discord.gg/SpmNs4S)
[![website](https://img.shields.io/badge/website-tauri.app-purple.svg)](https://tauri.app)
[![https://good-labs.github.io/greater-good-affirmation/assets/images/badge.svg](https://good-labs.github.io/greater-good-affirmation/assets/images/badge.svg)](https://good-labs.github.io/greater-good-affirmation)
[![support](https://img.shields.io/badge/sponsor-Open%20Collective-blue.svg)](https://opencollective.com/tauri)

Cross-platform WebView rendering library in Rust that supports all major desktop platforms like Windows, macOS, Linux, Android and iOS.

## Examples

This example leverages the `HasWindowHandle` trait from `raw-window-handle` crate and supports Windows, macOS, iOS, Android and Linux (X11 Only).
See the following example using `winit`.

```rs
use wry::{WebView, WebViewBuilder};
use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowId}};

#[derive(Default)]
struct App {
  webview_window: Option<(Window, WebView)>,
}

impl ApplicationHandler for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    let window = event_loop.create_window(Window::default_attributes()).unwrap();
    let webview = WebViewBuilder::new()
      .with_url("https://tauri.app")
      .build(&window)
      .unwrap();

    self.webview_window = Some((window, webview));
  }

  fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {}
}

let event_loop = EventLoop::new().unwrap();
let mut app = App::default();
event_loop.run_app(&mut app).unwrap();
```

If you also want to support Wayland too, then we recommend you use `WebViewBuilderExtUnix::new_gtk` on Linux.
See the following example using `tao`.

```rs
use wry::WebViewBuilder;
use tao::{window::WindowBuilder, event_loop::EventLoop};
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

let event_loop = EventLoop::new();
let window = WindowBuilder::new().build(&event_loop).unwrap();

let builder = WebViewBuilder::new().with_url("https://tauri.app");

#[cfg(not(target_os = "linux"))]
let webview = builder.build(&window).unwrap();
#[cfg(target_os = "linux")]
let webview = builder.build_gtk(window.gtk_window()).unwrap();
```

For more information, please read the crate [documentation](https://docs.rs/wry) .

## Platform-specific notes

Here is the underlying web engine each platform uses, and some dependencies you might need to install.

### Linux

Wry also needs [WebKitGTK](https://webkitgtk.org/) for WebView. So please make sure the following packages are installed:

#### Arch Linux / Manjaro:

```bash
sudo pacman -S webkit2gtk-4.1
```

#### Debian / Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev
```

#### Fedora

```bash
sudo dnf install gtk3-devel webkit2gtk4.1-devel
```

### Nix & NixOS

```Nix
# shell.nix

let
   # Unstable Channel | Rolling Release
   pkgs = import (fetchTarball("channel:nixpkgs-unstable")) { };
   packages = with pkgs; [
     pkg-config
     webkitgtk_4_1
   ];
 in
 pkgs.mkShell {
   buildInputs = packages;
 }
```

```sh
nix-shell shell.nix
```

#### GUIX

```scheme
;; manifest.scm

(specifications->manifest
  '("pkg-config"                ; Helper tool used when compiling
    "webkitgtk"                 ; Web content engine fot GTK+
 ))
```

```sh
guix shell -m manifest.scm
```

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

If you wish to create Android project yourself, there is a few requirements that your application needs to uphold:

1.  You need to set a few environment variables that will be used to generate the necessary kotlin
    files that you need to include in your Android application for wry to function properly:

    - `WRY_ANDROID_PACKAGE`: which is the reversed domain name of your android project and the app name in snake_case, for example, `com.wry.example.wry_app`
    - `WRY_ANDROID_LIBRARY`: for example, if your cargo project has a lib name `wry_app`, it will generate `libwry_app.so` so you set this env var to `wry_app`
    - `WRY_ANDROID_KOTLIN_FILES_OUT_DIR`: for example, `path/to/app/src/main/kotlin/com/wry/example`

2.  Your main Android Activity needs to inherit `AppCompatActivity`, preferably it should use the generated `WryActivity` or inherit it.
3.  Your Rust app needs to call `wry::android_setup` function to setup the necessary logic to be able to create webviews later on.
4.  Your Rust app needs to call `wry::android_binding!` macro to setup the JNI functions that will be called by `WryActivity` and various other places.

It is recommended to use [`tao`](https://docs.rs/tao/latest/tao/) crate as it provides maximum compatibility with `wry`

```rs
#[cfg(target_os = "android")]
{
  tao::android_binding!(
      com_example,
      wry_app,
      WryActivity,
      wry::android_setup, // pass the wry::android_setup function to tao which will invoke when the event loop is created
      _start_app
  );
  wry::android_binding!(com_example, ttt);
}
```

- `WRY_ANDROID_PACKAGE` which is the reversed domain name of your android project and the app name in snake_case for example: `com.wry.example.wry_app`
- `WRY_ANDROID_LIBRARY` for example: if your cargo project has a lib name `wry_app`, it will generate `libwry_app.so` so you set this env var to `wry_app`
- `WRY_ANDROID_KOTLIN_FILES_OUT_DIR` for example: `path/to/app/src/main/kotlin/com/wry/example`

## Partners

<table>
  <tbody>
    <tr>
      <td align="center" valign="middle">
        <a href="https://crabnebula.dev" target="_blank">
          <img src=".github/sponsors/crabnebula.svg" alt="CrabNebula" width="283">
        </a>
      </td>
    </tr>
  </tbody>
</table>

For the complete list of sponsors please visit our [website](https://tauri.app#sponsors) and [Open Collective](https://opencollective.com/tauri).

## License

Apache-2.0/MIT
