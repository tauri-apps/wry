#[derive(Debug, Clone)]
pub enum ProxyType {
  /// HTTP CONNECT
  Http,
  /// SOCKSv5
  Socks5,
}
#[derive(Debug, Clone)]
pub struct ProxyEndpoint {
  /// Proxy server host (e.g. 192.168.0.100, localhost, example.com, etc.)
  pub host: String,
  /// Proxy server port (e.g. 1080, 3128, etc.)
  pub port: String,
}
#[derive(Debug, Clone)]
pub enum ProxyConnection {
  /// Connect to proxy server via HTTP CONNECT
  Http(ProxyEndpoint),
  /// Connect to proxy server via SOCKSv5
  Socks5(ProxyEndpoint),
}
#[derive(Debug, Clone)]
pub struct ProxyConfig {
  pub proxy_type: ProxyType,
  pub proxy_connection: ProxyConnection,
}
