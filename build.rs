// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() {
  let is_macos = std::env::var("TARGET")
    .map(|t| t.ends_with("-darwin"))
    .unwrap_or_default();
  if is_macos {
    println!("cargo:rustc-link-lib=framework=WebKit");
  }

  let is_android = std::env::var("TARGET")
    .map(|t| t.ends_with("-android"))
    .unwrap_or_default();
  if is_android {
    use std::{fs, path::PathBuf};

    fn env_var(var: &str) -> String {
      std::env::var(var).expect(&format!(
        " `{}` is not set, which is needed to generate the kotlin files for android.",
        var
      ))
    }

    println!("cargo:rerun-if-env-changed=WRY_ANDROID_REVERSED_DOMAIN");
    println!("cargo:rerun-if-env-changed=WRY_ANDROID_APP_NAME_SNAKE_CASE");
    println!("cargo:rerun-if-env-changed=WRY_ANDROID_KOTLIN_FILES_OUT_DIR");

    let reversed_domain = env_var("WRY_ANDROID_REVERSED_DOMAIN");
    let app_name_snake_case = env_var("WRY_ANDROID_APP_NAME_SNAKE_CASE");
    let kotlin_out_dir = env_var("WRY_ANDROID_KOTLIN_FILES_OUT_DIR");

    let kotlin_out_dir = PathBuf::from(kotlin_out_dir)
      .canonicalize()
      .expect("Failed to canonicalize path");
    let kotlin_files =
      fs::read_dir(PathBuf::from(env_var("CARGO_MANIFEST_DIR")).join("src/webview/android/kotlin"))
        .expect("failed to read kotlin directory");

    for file in kotlin_files {
      let file = file.unwrap();

      let class_extension_env = format!(
        "WRY_{}_CLASS_EXTENSION",
        file.path().file_stem().unwrap().to_string_lossy()
      );

      println!("cargo:rerun-if-env-changed={}", class_extension_env);

      let content = fs::read_to_string(file.path())
        .expect("failed to read kotlin file as string")
        .replace("{{app-domain-reversed}}", &reversed_domain)
        .replace("{{app-name-snake-case}}", &app_name_snake_case)
        .replace(
          "{{class-extension}}",
          &std::env::var(&class_extension_env).unwrap_or_default(),
        );

      fs::write(kotlin_out_dir.join(file.file_name()), content)
        .expect("Failed to write kotlin file");
    }
  }
}
