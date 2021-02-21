#[cfg(target_os = "linux")]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    println!("cargo:rustc-link-lib=framework=WebKit");
}

#[cfg(target_os = "windows")]
fn main() {}
