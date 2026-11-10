// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Authentication challenge handling for mTLS (mutual TLS) connections.

use objc2::runtime::AnyObject;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::DeclaredClass;
use objc2_foundation::{
  NSData, NSString,
  NSURLAuthenticationChallenge, NSURLCredential,
  NSURLSessionAuthChallengeDisposition,
};

use super::class::wry_navigation_delegate::WryNavigationDelegate;

#[link(name = "Security", kind = "framework")]
extern "C" {
  fn SecCertificateCreateWithData(
    allocator: *const std::ffi::c_void,
    data: *const AnyObject,
  ) -> *mut std::ffi::c_void;
  fn SecTrustSetAnchorCertificates(
    trust: *const std::ffi::c_void,
    anchors: *const AnyObject,
  ) -> i32;
  fn SecTrustSetAnchorCertificatesOnly(
    trust: *const std::ffi::c_void,
    only: bool,
  ) -> i32;
  fn SecTrustEvaluateWithError(
    trust: *const std::ffi::c_void,
    error: *mut *mut std::ffi::c_void,
  ) -> bool;
  fn SecPKCS12Import(
    pkcs12: *const AnyObject,
    options: *const AnyObject,
    items: *mut *mut AnyObject,
  ) -> i32;
  fn CFRelease(cf: *const std::ffi::c_void);
}

pub(crate) fn did_receive_authentication_challenge(
  delegate: &WryNavigationDelegate,
  challenge: &NSURLAuthenticationChallenge,
  handler: &block2::Block<
    dyn Fn(NSURLSessionAuthChallengeDisposition, *mut NSURLCredential),
  >,
) {
  unsafe {
    let protection_space = challenge.protectionSpace();
    let auth_method = protection_space.authenticationMethod();

    let server_trust_method = NSString::from_str("NSURLAuthenticationMethodServerTrust");
    let client_cert_method = NSString::from_str("NSURLAuthenticationMethodClientCertificate");

    // Server trust challenge: pin CA cert if provided
    if auth_method.isEqualToString(&server_trust_method) {
      if let Some(ref ca_der) = delegate.ivars().trusted_ca_certificate {
        let ns_data = NSData::with_bytes(ca_der);
        let ca_cert = SecCertificateCreateWithData(
          std::ptr::null(),
          Retained::as_ptr(&ns_data) as *const AnyObject,
        );

        if !ca_cert.is_null() {
          let server_trust: *const std::ffi::c_void =
            msg_send![&*protection_space, serverTrust];
          if !server_trust.is_null() {
            let cert_obj = ca_cert as *mut AnyObject;
            let array: Retained<AnyObject> = msg_send![
              objc2::runtime::AnyClass::get(c"NSArray").unwrap(),
              arrayWithObject: cert_obj
            ];
            SecTrustSetAnchorCertificates(
              server_trust,
              Retained::as_ptr(&array) as *const AnyObject,
            );
            SecTrustSetAnchorCertificatesOnly(server_trust, true);

            let mut error: *mut std::ffi::c_void = std::ptr::null_mut();
            let trusted = SecTrustEvaluateWithError(server_trust, &mut error);
            CFRelease(ca_cert); // Release the SecCertificateRef

            if trusted {
              let credential: *mut NSURLCredential = msg_send![
                objc2::runtime::AnyClass::get(c"NSURLCredential").unwrap(),
                credentialForTrust: server_trust
              ];
              handler.call((
                NSURLSessionAuthChallengeDisposition::UseCredential,
                credential,
              ));
              return;
            }
            // Trust evaluation failed with pinned CA; cancel the challenge
            // rather than falling through to accept an untrusted server.
            handler.call((
              NSURLSessionAuthChallengeDisposition::CancelAuthenticationChallenge,
              std::ptr::null_mut(),
            ));
            return;
          }
          CFRelease(ca_cert);
        }
      }

      // No custom CA configured: use default system trust evaluation.
      // This preserves the standard WKWebView behavior of rejecting
      // untrusted/self-signed certificates.
      handler.call((
        NSURLSessionAuthChallengeDisposition::PerformDefaultHandling,
        std::ptr::null_mut(),
      ));
      return;
    }

    // Client certificate challenge: extract identity from PKCS#12 data
    if auth_method.isEqualToString(&client_cert_method) {
      if let Some(ref p12_data) = delegate.ivars().client_certificate_p12 {
        let password = delegate
          .ivars()
          .client_certificate_password
          .as_deref()
          .unwrap_or("");
        let ns_data = NSData::with_bytes(p12_data);
        let ns_password = NSString::from_str(password);

        // kSecImportExportPassphrase = "passphrase"
        let passphrase_key = NSString::from_str("passphrase");
        let options: Retained<AnyObject> = msg_send![
          objc2::runtime::AnyClass::get(c"NSDictionary").unwrap(),
          dictionaryWithObject: &*ns_password,
          forKey: &*passphrase_key
        ];

        let mut items: *mut AnyObject = std::ptr::null_mut();
        let status = SecPKCS12Import(
          Retained::as_ptr(&ns_data) as *const AnyObject,
          Retained::as_ptr(&options) as *const AnyObject,
          &mut items,
        );

        if status == 0 && !items.is_null() {
          let count: usize = msg_send![items, count];
          if count > 0 {
            let first: *mut AnyObject = msg_send![items, objectAtIndex: 0usize];
            // kSecImportItemIdentity = "identity"
            let identity_key = NSString::from_str("identity");
            let identity: *mut std::ffi::c_void =
              msg_send![first, objectForKey: &*identity_key];

            if !identity.is_null() {
              let credential: *mut NSURLCredential = msg_send![
                objc2::runtime::AnyClass::get(c"NSURLCredential").unwrap(),
                credentialWithIdentity: identity,
                certificates: std::ptr::null::<AnyObject>(),
                persistence: 0isize  // NSURLCredentialPersistenceNone
              ];
              CFRelease(items as *const std::ffi::c_void);
              handler.call((
                NSURLSessionAuthChallengeDisposition::UseCredential,
                credential,
              ));
              return;
            }
          }
          CFRelease(items as *const std::ffi::c_void);
        }
      }
    }

    // Default handling for all other challenges
    handler.call((
      NSURLSessionAuthChallengeDisposition::PerformDefaultHandling,
      std::ptr::null_mut(),
    ));
  }
}
