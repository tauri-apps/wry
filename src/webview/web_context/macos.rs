use crate::Result;

use super::WebContextData;

#[derive(Debug)]
pub struct WebContextImpl {
  protocols: Vec<*mut Box<dyn Fn(&str) -> Result<(Vec<u8>, String)>>>,
}

impl WebContextImpl {
  pub(crate) fn new(_data: &WebContextData) -> Self {
    Self {
      protocols: Vec::new(),
    }
  }

  pub(crate) fn set_allows_automation(&mut self, _flag: bool) {}

  pub(crate) fn registered_protocols(
    &mut self,
    handler: *mut Box<dyn Fn(&str) -> Result<(Vec<u8>, String)>>,
  ) {
    self.protocols.push(handler);
  }
}

impl Drop for WebContextImpl {
  fn drop(&mut self) {
    // We need to drop handler closures here
    unsafe {
      for ptr in self.protocols.iter() {
        if !ptr.is_null() {
          let _ = Box::from_raw(*ptr);
        }
      }
    }
  }
}
