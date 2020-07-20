#[cfg(target_os = "windows")]
winrt::build!(
    dependencies
        os
    types
        windows::foundation::*
        windows::web::ui::*
        windows::web::ui::interop::*
);

fn main() {
    #[cfg(target_os = "windows")]
    build();
}
