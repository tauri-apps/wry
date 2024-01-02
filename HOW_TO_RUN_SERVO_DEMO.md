# How to run Servo demo

This document describes how to run Servo demo on macOS.

- Build Servo

  - Clone Servo repository (rev@ 7305c59)

  ```sh
  git clone https://github.com/servo/servo.git
  cd servo
  git checkout 7305c59
  ```

  - Install LLVM from homebrew

  ```sh
  brew install llvm
  ```

  - Follow the instruction in [Servo - Build Setup (macOS)](https://github.com/servo/servo#macos)

- Build wry

  - Clone wry repository

  ```sh
  git clone https://github.com/tauri-apps/wry.git
  cd wry
  ```

  - Copy required files from Servo repository

  ```sh
  cp -a ../servo/resources .
  cp -f ../servo/Cargo.lock .
  ```

  - Build wry

  ```sh
  CC=/opt/homebrew/opt/llvm/bin/clang CXX=/opt/homebrew/opt/llvm/bin/clang++ cargo build
  ```

  - Run servo example

  ```sh
  CC=/opt/homebrew/opt/llvm/bin/clang CXX=/opt/homebrew/opt/llvm/bin/clang++ cargo run --example servo
  ```
