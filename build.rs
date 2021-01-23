fn main() {
    let target = std::env::var("TARGET").unwrap();
    if target.contains("-windows") {
        windows::build!(
            windows::foundation::*
            windows::web::ui::*
            windows::web::ui::interop::*
        );
    } else if target.contains("-apple") {
        println!("cargo:rustc-link-lib=framework=WebKit");
    }
}
