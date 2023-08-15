#[derive(Clone)]
pub enum ProxyType {
  Http,
  Socks5,
}
#[derive(Clone)]
pub struct ProxyEndpoint {
  pub host: String,
  pub port: String,
}
#[derive(Clone)]
pub enum ProxyConnection {
  Http(ProxyEndpoint),
  Socks5(ProxyEndpoint),
}
#[derive(Clone)]
pub struct ProxyConfig {
  pub proxy_type: ProxyType,
  pub proxy_connection: ProxyConnection,
}
