# WRY (Webview Rendering librarY)

[![](https://img.shields.io/crates/v/wry?style=flat-square)](https://crates.io/crates/wry) [![](https://img.shields.io/docsrs/wry?style=flat-square)](https://docs.rs/wry/) ![](https://img.shields.io/crates/l/wry?style=flat-square)

Cross-platfrom WebView rendering library in Rust that supports all major desktop platforms like Windows 10, macOS, and Linux.

```toml
[dependencies]
wry = "0.5.0"
```

<div align="center">
  <a href="https://gfycat.com/needywetelk">
    <img src="https://thumbs.gfycat.com/NeedyWetElk-size_restricted.gif">
  </a>
</div>

## Overview

Wry connects the web engine on each platform and provides easy to use and unified interface to render WebView. It uses
[winit] on most platforms and [gtk-rs] on Linux for windows creation.

[winit]: https://crates.io/crates/winit
[gtk-rs]: https://crates.io/crates/gtk

## Usage

The minimum example looks like following:

```rust
use wry::{Application, Result};

fn main() -> Result<()> {
    let mut app = Application::new()?;
    app.add_window(Default::default(), None)?;
    app.run();
    Ok(())
}
```

There are also more samples under `examples`, you can enter commands like following to try them:

```
cargo run --example multiwindow
```

For more information, please read the documentation below.

## Rust <-> Javascript

Communication between the host Rust code and webview Javascript is done via [JSON-RPC][].

Embedding code should call `set_handler()` on the application to register an incoming request handler and reply with responses that are passed back to Javascript. On the Javascript side the client is exposed via `window.rpc` with two public methods

1. The `call()` function accepts a method name and parameters and expects a reply.
2. The `notify()` function accepts a method name and parameters but does not expect a reply.

Both methods return promises but `notify()` resolves immediately.

For example in Rust:

```rust
use wry::{Application, Result, RpcResponse};

fn main() -> Result<()> {
    let mut app = Application::new()?;
    app.set_handler(Box::new(|proxy, mut req| {
      // Handle the request of type `RpcRequest` and reply with `RpcResponse`
      // Use the `WindowProxy` to modify the window, eg: `set_fullscreen` etc.
      None
    });
    app.add_window(Default::default())?;
    app.run();
    Ok(())
}
```

Then in Javascript use `call()` to call a remote method and get a response:

```javascript
async function callRemoteMethod() {
  let result = await window.rpc.call('remoteMethod', param1, param2);
  // Do something with the result
}
```

See the `rpc` example for more details.

## [Documentation](https://docs.rs/wry)

## Platform-specific notes

All platforms uses [winit](https://github.com/rust-windowing/winit) to build the window except Linux. Here are the underlying web engine each platfrom uses and some dependencies you might need to install.

### Linux

Unlike other platforms, [gtk-rs](https://gtk-rs.org/) is used to build the window instead of winit. Because wry needs [WebKitGTK](https://webkitgtk.org/) and winit provides lower level of interface like x11 or wayland. Please make sure WebKitGTK is installed. If not, run the following command:

#### Arch Linux / Manjaro:

```bash
sudo pacman -S webkit2gtk
```

#### Debian / Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.0-dev
```

### macOS

WebKit is native on macOS so everything should be fine.

If you are cross-compiling for macOS using [osxcross](https://github.com/tpoechtrager/osxcross) and encounter a runtime panic like `Class with name WKWebViewConfiguration could not be found` it's possible that `WebKit.framework` has not been linked correctly, to fix this set the `RUSTFLAGS` environment variable:

```
RUSTFLAGS="-l framework=WebKit" cargo build --target=x86_64-apple-darwin --release
```

### Windows

WebView2 provided by Microsoft Edge Chromium is used. So wry supports Windows 7, 8, and 10.

## License
Apache-2.0/MIT

[JSON-RPC]: https://www.jsonrpc.org
