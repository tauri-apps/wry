fn main() {
    let target = std::env::var("TARGET").unwrap();
    if target.contains("-windows") {
        windows::build!(
            windows::foundation::collections::*
            windows::foundation::{AsyncStatus, Rect, Uri}
            windows::web::ui::interop::{WebViewControl, WebViewControlProcess, }
        );

        let mut build = cc::Build::new();
        build
            .include("src/collections.h")
            .file("src/collections.cpp")
            .flag_if_supported("/std:c++17");

        println!("cargo:rerun-if-changed=src/collections.h");
        println!("cargo:rerun-if-changed=src/collections.cpp");
        build.compile("collections");
    } else if target.contains("-apple") {
        println!("cargo:rustc-link-lib=framework=WebKit");
    }
}
