# WRY (Webview Rendering librarY)

Cross-platfrom WebView rendering library in Rust that supports all major desktop platforms like Windows 10, macOS, and Linux.

```toml
[dependencies]
wry = "0.4.0"
```

## Overview

TODO

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

### Windows

We use EdgeHTML provided by Windows Runtime. So only Windows 10 is supported.

## License
Apache-2.0/MIT
