// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use cocoa::base::nil;
use std::ffi::c_char;

use crate::{proxy::ProxyEndpoint, Error};

use super::NSString;

#[allow(non_camel_case_types)]
pub type nw_endpoint_t = *mut objc::runtime::Object;
#[allow(non_camel_case_types)]
pub type nw_relay_hop_t = *mut objc::runtime::Object;
#[allow(non_camel_case_types)]
pub type nw_protocol_options_t = *mut objc::runtime::Object;
#[allow(non_camel_case_types)]
pub type nw_proxy_config_t = *mut objc::runtime::Object;

#[link(name = "Network", kind = "framework")]
extern "C" {
  #[allow(dead_code)]
  fn nw_endpoint_create_url(url: *const c_char) -> nw_endpoint_t;
  #[allow(dead_code)]
  fn nw_endpoint_get_url(endpoint: nw_endpoint_t) -> *const c_char;
  fn nw_endpoint_create_host(host: *const c_char, port: *const c_char) -> nw_endpoint_t;
  #[allow(dead_code)]
  fn nw_proxy_config_set_username_and_password(
    proxy_config: nw_proxy_config_t,
    username: *const c_char,
    password: *const c_char,
  );
  #[allow(dead_code)]
  fn nw_relay_hop_create(
    http3_relay_endpoint: nw_endpoint_t,
    http2_relay_endpoint: nw_endpoint_t,
    relay_tls_options: nw_protocol_options_t,
  ) -> nw_relay_hop_t;
  #[allow(dead_code)]
  fn nw_proxy_config_create_relay(
    first_hop: nw_relay_hop_t,
    second_hop: nw_relay_hop_t,
  ) -> nw_proxy_config_t;
  pub fn nw_proxy_config_create_socksv5(proxy_endpoint: nw_endpoint_t) -> nw_proxy_config_t;
  pub fn nw_proxy_config_create_http_connect(
    proxy_endpoint: nw_endpoint_t,
    proxy_tls_options: nw_protocol_options_t,
  ) -> nw_proxy_config_t;
}

impl TryFrom<ProxyEndpoint> for nw_endpoint_t {
  type Error = Error;
  fn try_from(endpoint: ProxyEndpoint) -> Result<Self, Error> {
    unsafe {
      let endpoint_host = NSString::new(&endpoint.host).to_cstr();
      let endpoint_port = NSString::new(&endpoint.port).to_cstr();
      let endpoint = nw_endpoint_create_host(endpoint_host, endpoint_port);

      match endpoint {
        #[allow(non_upper_case_globals)]
        nil => Err(Error::ProxyEndpointCreationFailed),
        _ => Ok(endpoint),
      }
    }
  }
}
