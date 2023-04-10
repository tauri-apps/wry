use crate::webview::webview2::encode_wide;
use once_cell::sync::Lazy;
use std::os::windows::ffi::OsStrExt;
use windows::{
  core::*,
  Win32::{
    Foundation::*,
    System::Com::{VARIANT, *},
  },
};
use windows_implement::implement;

static NAME: Lazy<Vec<u16>> = Lazy::new(|| {
  <str as AsRef<std::ffi::OsStr>>::as_ref("postMessage")
    .encode_wide()
    .collect()
});

#[implement(IDispatch)]
pub struct PostMessage;

impl IDispatch_Impl for PostMessage {
  fn GetTypeInfoCount(&self) -> Result<u32> {
    Ok(1)
  }

  fn GetTypeInfo(&self, _: u32, _: u32) -> Result<ITypeInfo> {
    Err(E_NOTIMPL.into())
  }

  fn GetIDsOfNames(
    &self,
    _riid: *const GUID,
    rgszNames: *const PCWSTR,
    cNames: u32,
    _lcid: u32,
    rgDispId: *mut i32,
  ) -> Result<()> {
    dbg!(cNames);
    dbg!(unsafe { (*rgszNames).to_string() });
    let name = unsafe { (*rgszNames).as_wide() };
    if cNames == 1 && NAME.as_slice() == name {
      Ok(())
    } else {
      Err(E_NOTIMPL.into())
    }
  }

  fn Invoke(
    &self,
    dispIdMember: i32,
    _riid: *const GUID,
    _lcid: u32,
    wFlags: DISPATCH_FLAGS,
    pDispParams: *const DISPPARAMS,
    pVarResult: *mut VARIANT,
    _pExcepInfo: *mut EXCEPINFO,
    _puArgErr: *mut u32,
  ) -> Result<()> {
    unsafe {
      dbg!(dispIdMember);
      dbg!(wFlags);
      dbg!(*pDispParams);
      dbg!((*pDispParams).rgvarg.offset(0));
    }
    Ok(())
  }
}

impl From<PostMessage> for VARIANT {
  fn from(obj: PostMessage) -> Self {
    Self {
      Anonymous: VARIANT_0 {
        Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
          vt: VT_DISPATCH,
          wReserved1: 0,
          wReserved2: 0,
          wReserved3: 0,
          Anonymous: VARIANT_0_0_0 {
            pdispVal: std::mem::ManuallyDrop::new(Some(obj.into())),
          },
        }),
      },
    }
  }
}
