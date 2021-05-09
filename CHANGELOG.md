# Changelog

## \[0.9.1]

- Correctly set visibilty when building `Window` on gtk-backend
  - [4395ad1](https://github.com/tauri-apps/wry/commit/4395ad147b799e67f9802c499346d0ad53554317) fix: only call `show_all` when needed ([#227](https://github.com/tauri-apps/wry/pull/227)) on 2021-05-02
- Fix `macOS` cursors and other minors UI glitch.
  - [d550b2f](https://github.com/tauri-apps/wry/commit/d550b2f0a1c708747537e3a5e6d880fea00e651d) fix(macOS): Window layers ([#220](https://github.com/tauri-apps/wry/pull/220)) on 2021-04-28
- Expose `print()` function to the webview. Work only on macOS for now.
  - [5206db6](https://github.com/tauri-apps/wry/commit/5206db6ca599fe0e146d72b04c908330e3045838) fix(macOS): Printing ([#235](https://github.com/tauri-apps/wry/pull/235)) ([#236](https://github.com/tauri-apps/wry/pull/236)) on 2021-05-06
- Fix macOS windows order for tray (statusbar) applications.
  - [229275f](https://github.com/tauri-apps/wry/commit/229275f106371d79800e0ca1cbc7b6c1827bc2ac) fix: macOS windows order ([#242](https://github.com/tauri-apps/wry/pull/242)) on 2021-05-07
- Add `request_redraw` method of `Window` on Linux
  - [03abfa0](https://github.com/tauri-apps/wry/commit/03abfa06019a78a182c7cd29dc63bf3d9df10e44) Add request_redraw method on Linux ([#222](https://github.com/tauri-apps/wry/pull/222)) on 2021-04-30
- Add tao as window dependency.
  - [483bad0](https://github.com/tauri-apps/wry/commit/483bad0fc7e7564500f7183547c15604fa387258) feat: tao as window dependency ([#230](https://github.com/tauri-apps/wry/pull/230)) on 2021-05-03
- Close the window when the instance is dropped on Linux and Windows.
  - [3f2cc28](https://github.com/tauri-apps/wry/commit/3f2cc28b4fbfcf54c97000a6541e9356440838e8) fix: close window when the instance is dropped ([#228](https://github.com/tauri-apps/wry/pull/228)) on 2021-05-02
- Remove winit dependency on Linux
  - [fa15076](https://github.com/tauri-apps/wry/commit/fa15076207d9e678db4149210aba929044d0ff45) feat: winit interface for gtk ([#163](https://github.com/tauri-apps/wry/pull/163)) on 2021-04-19
  - [39d6f59](https://github.com/tauri-apps/wry/commit/39d6f595d81c857e92aef31cc2559b402e64edd3) publish new versions ([#166](https://github.com/tauri-apps/wry/pull/166)) on 2021-04-29
  - [4ef8330](https://github.com/tauri-apps/wry/commit/4ef8330d856e07d34bf86d1f2903c82c37042556) Remove winit dependency on Linux ([#226](https://github.com/tauri-apps/wry/pull/226)) on 2021-04-30

## \[0.9.0]

- Refactor signatures of most closure types
  - [b8823fe](https://github.com/tauri-apps/wry/commit/b8823fe14ee5f95d07cd2cb1f9f673b964c9dc83) refactor: signature of closure types ([#167](https://github.com/tauri-apps/wry/pull/167)) on 2021-04-19
- Drop handler closures properly on macOS.
  - [f905503](https://github.com/tauri-apps/wry/commit/f905503c4a010ed4219c6ad36d14c0dbf0b6e122) fix: [#160](https://github.com/tauri-apps/wry/pull/160) drop handler closures properly ([#211](https://github.com/tauri-apps/wry/pull/211)) on 2021-04-27
- Fix `history.pushState` in webview2.
  - [dd0fa46](https://github.com/tauri-apps/wry/commit/dd0fa46494c1ab8536bcc7ea1dd16341b12856b4) Use http instead of file for windows custom protocol workaround ([#173](https://github.com/tauri-apps/wry/pull/173)) on 2021-04-20
- The `data_directory` field now affects the IndexedDB and LocalStorage directories on Linux.
  - [1a6c821](https://github.com/tauri-apps/wry/commit/1a6c8216ee6865ca14025c229b37342496b38f26) feat(linux): implement custom user data path ([#188](https://github.com/tauri-apps/wry/pull/188)) on 2021-04-22
- Fix runtime panic on macOS, when no file handler are defined.
  - [22a4991](https://github.com/tauri-apps/wry/commit/22a4991aa8ca7c75aa52150a90379c40bcc34d07) bug(macOS): Runtime panic when no file_drop_handler ([#177](https://github.com/tauri-apps/wry/pull/177)) on 2021-04-20
- Add position field on WindowAttribute
  - [2b3be7a](https://github.com/tauri-apps/wry/commit/2b3be7a4db2cbc1612c7105cb698c1f21a05da77) Add position field on WindowAttribute ([#219](https://github.com/tauri-apps/wry/pull/219)) on 2021-04-28
- Fix panic on mutiple custom protocol registration.
  - [01647a2](https://github.com/tauri-apps/wry/commit/01647a2a5b769bc192754c2d3806a55112d58d33) Fix custom protocol registry on mac ([#205](https://github.com/tauri-apps/wry/pull/205)) on 2021-04-26
- Fix SVG render with the custom protocol.
  - [890cfe5](https://github.com/tauri-apps/wry/commit/890cfe527996c181d643c9f8e5fc3e79ff0841a0) fix(custom-protocol): SVG mime type - close [#168](https://github.com/tauri-apps/wry/pull/168) ([#169](https://github.com/tauri-apps/wry/pull/169)) on 2021-04-19
- Initial custom WindowExtWindows trait.
  - [1ef1f58](https://github.com/tauri-apps/wry/commit/1ef1f58efb6afa6c6b9eda3a43ee83fc79c3b78e) feat: custom WindowExtWindow trait ([#191](https://github.com/tauri-apps/wry/pull/191)) on 2021-04-23
- Fix transparency on Windows
  - [e278556](https://github.com/tauri-apps/wry/commit/e2785566c69d43f003896b7b5da79b29d2966c13) fix: transparency on Windows  ([#217](https://github.com/tauri-apps/wry/pull/217)) on 2021-04-28
- Add platform module and WindowExtUnix trait on Linux
  - [004e298](https://github.com/tauri-apps/wry/commit/004e298e0198e6576a11e6e84fdf6b7c2f66b6ae) feat: WindowExtUnix trait ([#192](https://github.com/tauri-apps/wry/pull/192)) on 2021-04-23
- Make sure custom protocol on Windows is over HTTPS.
  - [c36db35](https://github.com/tauri-apps/wry/commit/c36db35b2b8704eb36bc341cd99abac01abfab87) fix(custom-protocol): Make sure custom protocol on Windows is over HTTPS. ([#179](https://github.com/tauri-apps/wry/pull/179)) on 2021-04-20
- Initial winit interface for gtk backend
  - [fa15076](https://github.com/tauri-apps/wry/commit/fa15076207d9e678db4149210aba929044d0ff45) feat: winit interface for gtk ([#163](https://github.com/tauri-apps/wry/pull/163)) on 2021-04-19

## \[0.8.0]

- Wry now accepts multiple custom protocol registerations.
  - [db64fc6](https://github.com/tauri-apps/wry/commit/db64fc69c48a728184fcef001688b94f0294edab) feat/licenses ([#155](https://github.com/tauri-apps/wry/pull/155)) on 2021-04-14
- Apply license header for SPDX compliance.
  - [05e0218](https://github.com/tauri-apps/wry/commit/05e02180c9fe929d3e691185df44257654546935) feat: multiple custom protocols ([#151](https://github.com/tauri-apps/wry/pull/151)) on 2021-04-11
  - [db64fc6](https://github.com/tauri-apps/wry/commit/db64fc69c48a728184fcef001688b94f0294edab) feat/licenses ([#155](https://github.com/tauri-apps/wry/pull/155)) on 2021-04-14
- Remove bindings crate and use windows-webview2 as dependency instead.
  - [c2156a4](https://github.com/tauri-apps/wry/commit/c2156a45d7fbfead956b6d03b2594962e3455e6d) Move to windows-webview2 as dependency for winrt impl ([#144](https://github.com/tauri-apps/wry/pull/144)) on 2021-04-03

## \[0.7.0]

- Add old win32 implementation on windows as default feature flag.
  - [1a88cd2](https://github.com/tauri-apps/wry/commit/1a88cd267f2a29c1dd35d7197250972718081847) refactor: Add win32 implementation and feature flag for both backends ([#139](https://github.com/tauri-apps/wry/pull/139)) on 2021-04-02
- Adds a `WindowProxy` to the file drop handler closure - `WindowFileDropHandler`.
  - [20cb051](https://github.com/tauri-apps/wry/commit/20cb051aba28009c70dad838b2a9b1575cb5363a) feat: add WindowProxy to file drop handler closure ([#140](https://github.com/tauri-apps/wry/pull/140)) on 2021-04-01

## \[0.6.2]

- Add pipe back to version check for covector config. This prevents the CI failure on publish if it exists already. The issue was patched in covector (and tests in place so it doesn't break in the future).
  - [a32829c](https://github.com/tauri-apps/wry/commit/a32829c527f02b228fa1da45e9710941c5415bfc) chore: add pipe for publish check back in ([#131](https://github.com/tauri-apps/wry/pull/131)) on 2021-03-28
- Fix messages to the webview from the backend being delayed on Linux/GTK when the user is not actively engaged with the UI.
  - [d2a2a9f](https://github.com/tauri-apps/wry/commit/d2a2a9f473d2588b27a95bf627d125caea1b979d) fix: spawn async event loop on gtk to prevent delayed messages ([#135](https://github.com/tauri-apps/wry/pull/135)) on 2021-03-31
- Add draggable regions, just add `drag-region` class to the html element.
  - [b2a0bfc](https://github.com/tauri-apps/wry/commit/b2a0bfc289786d0a23dac0c8d9543771e70e3427) feat/ draggable-region ([#92](https://github.com/tauri-apps/wry/pull/92)) on 2021-03-25
- Add event listener in application proxy
  - [c49846c](https://github.com/tauri-apps/wry/commit/c49846cfc41bb548a685edeac5f8036501f7dcec) feat: event listener ([#129](https://github.com/tauri-apps/wry/pull/129)) on 2021-03-26
- Better result errror handling
  - [485035f](https://github.com/tauri-apps/wry/commit/485035f17d28560966b07b512935821814f0e951) chore: better result error handling ([#124](https://github.com/tauri-apps/wry/pull/124)) on 2021-03-21
- Fix visibility on webview2 when window was invisible previously and then shown.
  - [6d31706](https://github.com/tauri-apps/wry/commit/6d31706a6bff43e9b28100675cf8fc12f29db248) Fix visibility on webview2 when window was invisible previously ([#128](https://github.com/tauri-apps/wry/pull/128)) on 2021-03-24

## \[0.6.1]

- Add attribute option to allow WebView on Windows use user_data folder
  - [8dd58ee](https://github.com/tauri-apps/wry/commit/8dd58eec77d4c89491b1af427d06c4ee6cfa8e58) feat/ allow webview2 (windows) to use optional user_data folder provided by the attributes ([#120](https://github.com/tauri-apps/wry/pull/120)) on 2021-03-21

## \[0.6.0]

- Initialize covector!
  - [33b64ed](https://github.com/tauri-apps/wry/commit/33b64ed5c208b778d03dbb5f3f2808bb417c9f52) chore: covector init ([#55](https://github.com/tauri-apps/wry/pull/55)) on 2021-02-21
- Support Windows 7, 8, and 10
  - [fbf0d17](https://github.com/tauri-apps/wry/commit/fbf0d17164da455400aaa44104c3925eded09393) Adopt Webview2 on Windows ([#48](https://github.com/tauri-apps/wry/pull/48)) on 2021-02-20
- Dev tools are enabled on debug build
- Add skip task bar option
  - [395b6fb](https://github.com/tauri-apps/wry/commit/395b6fbcd66f6cbd0457cb609bea4afe734fadd4) feat: `skip_taskbar` for windows ([#49](https://github.com/tauri-apps/wry/pull/49)) on 2021-02-20
- Add custom protocol option
  - [a492806](https://github.com/tauri-apps/wry/commit/7a492806d716a30abe15a2104b64152c1ca370bb) Add custom protocol ([#65](https://github.com/tauri-apps/wry/pull/65)) on 2021-02-23
- Add transparent option to mac and linux
- Error type has Send/Sync traits
  - [3536b83](https://github.com/tauri-apps/wry/commit/3536b831ec30ee7436616ba4b262bbdd1e6279c8) Add .changes file in prepare of v0.6 on 2021-02-24
- Replace Callback with RPC handler
  - [e215157](https://github.com/tauri-apps/wry/commit/e215157146f0eab8ee6beab0628b036c68eea108) Implement draft RPC API ([#95](https://github.com/tauri-apps/wry/pull/95)) on 2021-03-04
- Add File drop handlers
  - [fed0ee7](https://github.com/tauri-apps/wry/commit/fed0ee772100ad19a344a85266618c7bcf7cb649) File drop handlers ([#96](https://github.com/tauri-apps/wry/pull/96)) on 2021-03-09
