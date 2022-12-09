// Copyright 2020-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
if (window.location.pathname.startsWith('/custom_protocol_page2')) {
    console.log("hello from javascript in page2");
} else {
    console.log("hello from javascript in page1");

    WebAssembly.instantiateStreaming(fetch("/custom_protocol_wasm.wasm"))
        .then(wasm => {
            console.log(wasm.instance.exports.main()); // should log 42
        })
}
