#[macro_use]
extern crate thiserror;

extern crate serde;
extern crate serde_json;

fn main() -> webview2_nuget::Result<()> {
  let package_root = webview2_nuget::install()?;
  webview2_nuget::update_windows(package_root)?;

  windows::build!(
      Microsoft::Web::WebView2::Core::*,
      Windows::Foundation::*,
      Windows::Storage::Streams::*,
      Windows::Win32::Com::*,
      Windows::Win32::DisplayDevices::{
        POINT,
        POINTL,
        RECT,
        SIZE
      },
      Windows::Win32::Gdi::UpdateWindow,
      Windows::Win32::HiDpi::{
        PROCESS_DPI_AWARENESS,
        SetProcessDpiAwareness
      },
      Windows::Win32::KeyboardAndMouseInput::SetFocus,
      Windows::Win32::MenusAndResources::HMENU,
      Windows::Win32::Shell::{
        DragFinish,
        DragQueryFileW,
        HDROP,
        ITaskbarList,
        TaskbarList
      },
      Windows::Win32::SystemServices::{
        BOOL,
        CLIPBOARD_FORMATS,
        DRAGDROP_E_INVALIDHWND,
        DV_E_FORMATETC,
        GetCurrentThreadId,
        GetModuleHandleA,
        HINSTANCE,
        LRESULT,
        PWSTR,
        userHMETAFILEPICT,
        userHENHMETAFILE,
      },
      Windows::Win32::WindowsAndMessaging::*
  );

  Ok(())
}

mod webview2_nuget {
  use std::{convert::From, env, fs, io, path::PathBuf, process::Command};

  const WEBVIEW2_NAME: &str = "Microsoft.Web.WebView2";
  const WEBVIEW2_VERSION: &str = "1.0.824-prerelease";

  pub fn install() -> Result<PathBuf> {
    let manifest_dir = get_manifest_dir()?;
    let install_root = match manifest_dir.to_str() {
      Some(path) => path,
      None => return Err(Error::MissingPath(manifest_dir))
    };

    let mut package_root = manifest_dir.clone();
    package_root.push(format!("{}.{}", WEBVIEW2_NAME, WEBVIEW2_VERSION));

    if !check_nuget_dir(install_root)? {
      let mut nuget_path = manifest_dir.clone();
      nuget_path.push("tools");
      nuget_path.push("nuget.exe");

      let nuget_tool = match nuget_path.to_str() {
        Some(path) => path,
        None => return Err(Error::MissingPath(nuget_path))
      };

      Command::new(nuget_tool)
        .args(&[
          "install",
          WEBVIEW2_NAME,
          "-OutputDirectory",
          install_root,
          "-Version",
          WEBVIEW2_VERSION,
        ])
        .output()?;

      if !check_nuget_dir(install_root)? {
        return Err(Error::MissingPath(package_root))
      }
    }

    Ok(package_root)
  }

  fn get_manifest_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(env::var("CARGO_MANIFEST_DIR")?))
  }

  fn check_nuget_dir(install_root: &str) -> Result<bool> {
    let nuget_path = format!("{}.{}", WEBVIEW2_NAME, WEBVIEW2_VERSION);
    let mut dir_iter = fs::read_dir(install_root)?.filter(|dir| match dir {
      Ok(dir) => match dir.file_type() {
        Ok(file_type) => {
          file_type.is_dir()
            && match dir.file_name().to_str() {
              Some(name) => name.eq_ignore_ascii_case(&nuget_path),
              None => false,
            }
        }
        Err(_) => false,
      },
      Err(_) => false,
    });
    Ok(dir_iter.next().is_some())
  }

  pub fn update_windows(package_root: PathBuf) -> Result<()> {
    const WEBVIEW2_WINMD: &str = "Microsoft.Web.WebView2.Core.winmd";

    let mut windows_dir = get_workspace_dir()?;
    windows_dir.push(".windows");

    let mut winmd_dest = windows_dir.clone();
    winmd_dest.push("winmd");
    winmd_dest.push(WEBVIEW2_WINMD);
    let mut winmd_src = package_root.clone();
    winmd_src.push("lib");
    winmd_src.push(WEBVIEW2_WINMD);
    eprintln!("Copy from {:?} -> {:?}", winmd_src, winmd_dest);
    fs::copy(winmd_src.as_path(), winmd_dest.as_path())?;

    const WEBVIEW2_DLL: &str = "Microsoft.Web.WebView2.Core.dll";
    const WEBVIEW2_TARGETS: &[&'static str] = &["arm64", "x64", "x86"];

    let mut runtimes_dir = package_root;
    runtimes_dir.push("runtimes");
    for target in WEBVIEW2_TARGETS {
      let mut dll_dest = windows_dir.clone();
      dll_dest.push(target);
      dll_dest.push(WEBVIEW2_DLL);
      let mut dll_src = runtimes_dir.clone();
      dll_src.push(format!("win-{}", target));
      dll_src.push("native_uap");
      dll_src.push(WEBVIEW2_DLL);
      eprintln!("Copy from {:?} -> {:?}", dll_src, dll_dest);
      fs::copy(dll_src.as_path(), dll_dest.as_path())?;
    }

    Ok(())
  }

  fn get_workspace_dir() -> Result<PathBuf> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct CargoMetadata {
      workspace_root: String
    }

    let output = Command::new(env::var("CARGO")?)
      .args(&[
        "metadata",
        "--format-version=1",
        "--no-deps",
        "--offline",
      ])
      .output()?;

    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)?;

    Ok(PathBuf::from(metadata.workspace_root))
  }

  #[derive(Debug, Error)]
  pub enum Error {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    VarError(#[from] env::VarError),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
    #[error("Missing Path")]
    MissingPath(PathBuf),
  }

  pub type Result<T> = std::result::Result<T, Error>;
}
