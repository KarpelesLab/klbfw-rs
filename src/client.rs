/// Configuration for REST API client
#[derive(Debug, Clone)]
pub struct Config {
    /// URL scheme (http or https)
    scheme: String,
    /// API host (may include a `:port` suffix)
    host: String,
    /// Enable debug logging
    debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            scheme: "https".to_string(),
            host: "www.atonline.com".to_string(),
            debug: false,
        }
    }
}

impl Config {
    /// Create a new configuration with the given scheme and host
    pub fn new(scheme: String, host: String) -> Self {
        Config {
            scheme,
            host,
            debug: false,
        }
    }

    /// Set debug mode (builder style)
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Set debug mode in place
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    /// URL scheme (http or https)
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// API host (may include a `:port` suffix)
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Whether debug logging is enabled
    pub fn debug(&self) -> bool {
        self.debug
    }

    /// Get the base URL for API requests.
    ///
    /// Non-ASCII hostnames are IDNA-encoded (punycode); a `:port` suffix is
    /// preserved as-is.
    pub fn base_url(&self) -> String {
        format!("{}://{}", self.scheme, self.encoded_host())
    }

    /// IDNA-encode the host, leaving an optional `:port` suffix untouched.
    fn encoded_host(&self) -> String {
        // Split an optional numeric port suffix off the host.
        let (host, port) = match self.host.rsplit_once(':') {
            Some((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => (h, Some(p)),
            _ => (self.host.as_str(), None),
        };

        let encoded = if host.is_ascii() {
            host.to_string()
        } else {
            // Fall back to the raw host if IDNA encoding fails.
            intl::unicode::idna::to_ascii(host).unwrap_or_else(|_| host.to_string())
        };

        match port {
            Some(p) => format!("{}:{}", encoded, p),
            None => encoded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_default() {
        let config = Config::default();
        assert_eq!(config.base_url(), "https://www.atonline.com");
    }

    #[test]
    fn test_base_url_with_port() {
        let config = Config::new("http".to_string(), "localhost:8080".to_string());
        assert_eq!(config.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_base_url_idna() {
        let config = Config::new("https".to_string(), "bücher.example".to_string());
        assert_eq!(config.base_url(), "https://xn--bcher-kva.example");
    }
}
