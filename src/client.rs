use reqwest::blocking::{Client, ClientBuilder};
use std::time::Duration;

/// Create the default HTTP client for REST API requests
/// with optimized settings for connection pooling and timeouts
pub fn create_rest_client() -> Client {
    ClientBuilder::new()
        .pool_max_idle_per_host(50)
        .timeout(Duration::from_secs(300)) // 5 minutes
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client")
}

/// Create the HTTP client for upload requests with longer timeout
pub fn create_upload_client() -> Client {
    ClientBuilder::new()
        .pool_max_idle_per_host(50)
        .timeout(Duration::from_secs(3600)) // 1 hour
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create upload HTTP client")
}

/// Configuration for REST API client
#[derive(Debug, Clone)]
pub struct Config {
    /// URL scheme (http or https)
    pub scheme: String,
    /// API host
    pub host: String,
    /// Enable debug logging
    pub debug: bool,
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

    /// Set debug mode
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Get the base URL for API requests
    pub fn base_url(&self) -> String {
        format!("{}://{}", self.scheme, self.host)
    }
}
