pub enum ProxyType {
  Http,
  Socks5,
}

pub struct ProxyEndpoint {
  pub host: String,
  pub port: String,
}

pub enum ProxyConnection {
  Http(ProxyEndpoint),
  Socks5(ProxyEndpoint),
}

pub struct ProxyConfig {
  pub proxy_type: ProxyType,
  pub proxy_connection: ProxyConnection,
}
