use std::ptr::NonNull;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_foundation::{NSArray, NSError, NSUUID};
use objc2_web_kit::WKWebsiteDataStore;

use crate::Error;

/// Fetches all Data Store Identifiers of this application
///
/// Needs to run on main thread and needs an event loop to run.
pub fn fetch_all_data_store_identifiers(
  cb: impl Fn(Vec<[u8; 16]>) + Send + 'static,
) -> Result<(), Error> {
  let block = RcBlock::new(move |stores: NonNull<NSArray<NSUUID>>| {
    let uuid_list = unsafe { stores.as_ref() }
      .to_vec()
      .iter()
      .map(|uuid| uuid.as_bytes())
      .collect();
    cb(uuid_list);
  });

  match MainThreadMarker::new() {
    Some(mtn) => unsafe {
      WKWebsiteDataStore::fetchAllDataStoreIdentifiers(&block, mtn);
      Ok(())
    },
    None => Err(Error::NotMainThread),
  }
}

/// Deletes a Data Store by Identifiers
///
/// Needs to run on main thread and needs an event loop to run.
pub fn remove_data_store(
  uuid: &[u8; 16],
  cb: impl Fn(Result<(), Error>) + Send + 'static,
) -> Result<(), Error> {
  let mtm = MainThreadMarker::new().ok_or(Error::NotMainThread)?;
  let identifier = NSUUID::from_bytes(uuid.to_owned());

  let block = RcBlock::new(move |error: *mut NSError| {
    if error.is_null() {
      cb(Ok(()))
    } else {
      cb(Err(unsafe { error.read() }.into()));
    }
  });

  unsafe {
    WKWebsiteDataStore::removeDataStoreForIdentifier_completionHandler(&identifier, &block, mtm);
  }

  Ok(())
}
