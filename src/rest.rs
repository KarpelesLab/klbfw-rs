use crate::apikey::ApiKey;
use crate::client::Config;
use crate::error::{RestError, Result};
use crate::response::Response;
use crate::token::Token;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Overall request timeout for REST calls.
const REST_TIMEOUT: Duration = Duration::from_secs(300);
/// Connection establishment timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Client for REST API requests.
///
/// Holds the configuration, optional authentication (token or API key), and any
/// custom headers, and exposes methods to make requests.
#[derive(Clone)]
pub struct Client {
    /// Configuration
    config: Config,
    /// Optional authentication token (shared so renewals persist across calls)
    token: Arc<Mutex<Option<Token>>>,
    /// Optional API key
    api_key: Option<ApiKey>,
    /// Extra headers applied to every request (in insertion order)
    headers: Vec<(String, String)>,
}

impl Client {
    /// Create a new REST context with default configuration
    pub fn new() -> Self {
        Client {
            config: Config::default(),
            token: Arc::new(Mutex::new(None)),
            api_key: None,
            headers: Vec::new(),
        }
    }

    /// Create a new REST context with custom configuration
    pub fn with_config(config: Config) -> Self {
        Client {
            config,
            token: Arc::new(Mutex::new(None)),
            api_key: None,
            headers: Vec::new(),
        }
    }

    /// Set the authentication token
    pub fn with_token(self, token: Token) -> Self {
        *self.token.lock().unwrap() = Some(token);
        self
    }

    /// Set the API key
    pub fn with_api_key(mut self, api_key: ApiKey) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Add a custom header applied to every request (builder style).
    ///
    /// Custom headers are sent in addition to the headers the client sets
    /// automatically (`Authorization`, `Content-Type`, ...); they do not
    /// replace them. Call multiple times to add several headers, including
    /// repeated header names.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Add several custom headers applied to every request (builder style).
    ///
    /// Headers are appended to any already set; see [`with_header`](Self::with_header).
    pub fn with_headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.headers
            .extend(headers.into_iter().map(|(k, v)| (k.into(), v.into())));
        self
    }

    /// Add a custom header applied to every request (in place).
    pub fn set_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.headers.push((name.into(), value.into()));
    }

    /// The custom headers configured on this context.
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Enable debug mode
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.config.set_debug(debug);
        self
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Make a REST API request and unmarshal the response data into the target type
    ///
    /// # Arguments
    /// * `path` - API endpoint path
    /// * `method` - HTTP method (GET, POST, PUT, etc.)
    /// * `param` - Request parameters or body content
    ///
    /// # Returns
    /// The unmarshaled response data of type T
    pub fn apply<T, P>(&self, path: &str, method: &str, param: P) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        P: Serialize,
    {
        let response = self.do_request(path, method, param)?;
        response.apply()
    }

    /// Execute a REST API request and return the raw Response object
    ///
    /// # Arguments
    /// * `path` - API endpoint path
    /// * `method` - HTTP method (GET, POST, PUT, etc.)
    /// * `param` - Request parameters or body content
    ///
    /// # Returns
    /// The raw Response object
    pub fn do_request<P>(&self, path: &str, method: &str, param: P) -> Result<Response>
    where
        P: Serialize,
    {
        let param_json = serde_json::to_value(param)?;
        self.request_inner(path, method, &param_json, true)
    }

    /// Inner request implementation.
    ///
    /// `allow_renew` guards token renewal so an expired token triggers exactly
    /// one retry.
    fn request_inner(
        &self,
        path: &str,
        method: &str,
        param_json: &serde_json::Value,
        allow_renew: bool,
    ) -> Result<Response> {
        // Build base URL
        let base_url = self.config.base_url();
        let url = format!("{}/_special/rest/{}", base_url, path);

        let mut query_params: HashMap<String, String> = HashMap::new();
        let mut body_bytes: Vec<u8> = Vec::new();

        match method {
            "GET" | "HEAD" | "OPTIONS" => {
                // Parameters go in query string
                let param_str = serde_json::to_string(param_json)?;
                query_params.insert("_".to_string(), param_str);
            }
            "PUT" | "POST" | "PATCH" => {
                // Parameters go in request body
                body_bytes = serde_json::to_vec(param_json)?;
            }
            "DELETE" => {
                // No parameters
            }
            _ => {
                return Err(RestError::RequestBuild(format!(
                    "Unsupported HTTP method: {}",
                    method
                )))
            }
        }

        // Apply API key authentication if present
        if let Some(ref api_key) = self.api_key {
            api_key.apply_params(method, path, &mut query_params, &body_bytes)?;
        }

        // Build the full URL with an (optional) query string.
        let full_url = if query_params.is_empty() {
            url
        } else {
            let query = form_urlencoded::Serializer::new(String::new())
                .extend_pairs(query_params.iter())
                .finish();
            format!("{}?{}", url, query)
        };

        // Snapshot the current token (used only when not authenticating by key).
        let current_token = if self.api_key.is_none() {
            self.token.lock().unwrap().clone()
        } else {
            None
        };

        // Build the request.
        let mut request = rsurl::Request::new(method, &full_url)?
            .header("Sec-Rest-Http", "false")
            .max_time(REST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT);

        // Apply user-supplied custom headers before the client-managed ones so
        // that Authorization/Content-Type set below take precedence.
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }

        if let Some(ref token) = current_token {
            request = request.header("Authorization", &format!("Bearer {}", token.access_token));
        }

        if !body_bytes.is_empty() {
            request = request
                .header("Content-Type", "application/json")
                .body(body_bytes);
        }

        // Execute request
        let start = std::time::Instant::now();
        let http_response = request.send()?;
        let status = http_response.status;

        // Get X-Request-Id header
        let request_id = http_response.header("X-Request-Id").map(|s| s.to_string());

        let body = http_response.body;

        if self.config.debug() {
            let duration = start.elapsed();
            eprintln!(
                "[rest] {} {} => {:?} (status: {})",
                method, path, duration, status
            );
        }

        // Parse response
        let mut response: Response = serde_json::from_slice(&body).map_err(|e| {
            if !(200..400).contains(&status) {
                RestError::http(
                    status,
                    String::from_utf8_lossy(&body).to_string(),
                    Some(Box::new(e)),
                )
            } else {
                RestError::Json(e)
            }
        })?;

        response.request_id = request_id;

        // Check for token expiration and renew if needed
        if allow_renew {
            if let Some(token) = current_token {
                if response.token.as_deref() == Some("invalid_request_token")
                    && response.extra.as_deref() == Some("token_expired")
                {
                    if self.config.debug() {
                        eprintln!("[rest] Token expired, attempting renewal");
                    }

                    // Renew and persist the new token so later calls reuse it.
                    let renewed = self.renew_token(&token)?;
                    *self.token.lock().unwrap() = Some(renewed);

                    // Retry the request once with the renewed token.
                    return self.request_inner(path, method, param_json, false);
                }
            }
        }

        // Check for redirect
        if response.result == "redirect" {
            if response.exception.as_deref() == Some("Exception\\Login") {
                return Err(RestError::LoginRequired);
            }
            return Err(RestError::from_response(response));
        }

        // Check for error response
        if response.result == "error" {
            return Err(RestError::from_response(response));
        }

        Ok(response)
    }

    /// Renew an expired token, returning the renewed token.
    fn renew_token(&self, token: &Token) -> Result<Token> {
        if !token.has_client_id() {
            return Err(RestError::NoClientId);
        }
        if !token.has_refresh_token() {
            return Err(RestError::NoRefreshToken);
        }

        // Create a context without token to avoid recursion, preserving any
        // custom headers so they apply to the renewal request too.
        let ctx = Client {
            config: self.config.clone(),
            token: Arc::new(Mutex::new(None)),
            api_key: None,
            headers: self.headers.clone(),
        };

        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("client_id", &token.client_id);
        params.insert("refresh_token", &token.refresh_token);
        params.insert("noraw", "true");

        let mut renewed: Token = ctx.apply("OAuth2:token", "POST", params)?;

        // The renewal response does not echo the client_id; carry it over so
        // the token remains renewable.
        renewed.client_id = token.client_id.clone();

        Ok(renewed)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// Deprecated alias for [`Client`].
///
/// The type was renamed to [`Client`] to better match Rust conventions; this
/// alias keeps existing code compiling.
#[deprecated(
    since = "0.1.3",
    note = "renamed to `Client`; use `klbfw::Client` instead"
)]
pub type RestContext = Client;

/// Convenience function to create a new REST context and make a request
pub fn apply<T, P>(path: &str, method: &str, param: P) -> Result<T>
where
    T: serde::de::DeserializeOwned,
    P: Serialize,
{
    Client::new().apply(path, method, param)
}

/// Convenience function to create a new REST context and execute a request
pub fn do_request<P>(path: &str, method: &str, param: P) -> Result<Response>
where
    P: Serialize,
{
    Client::new().do_request(path, method, param)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rest_context_creation() {
        let ctx = Client::new();
        assert_eq!(ctx.config().scheme(), "https");
        assert_eq!(ctx.config().host(), "www.atonline.com");
    }

    #[test]
    fn test_rest_context_with_config() {
        let config = Config::new("http".to_string(), "localhost:8080".to_string());
        let ctx = Client::with_config(config);
        assert_eq!(ctx.config().scheme(), "http");
        assert_eq!(ctx.config().host(), "localhost:8080");
    }

    #[test]
    fn test_custom_headers() {
        let ctx = Client::new()
            .with_header("X-Custom", "one")
            .with_headers([("X-A", "a"), ("X-B", "b")]);
        assert_eq!(
            ctx.headers(),
            &[
                ("X-Custom".to_string(), "one".to_string()),
                ("X-A".to_string(), "a".to_string()),
                ("X-B".to_string(), "b".to_string()),
            ]
        );

        let mut ctx = ctx;
        ctx.set_header("X-C", "c");
        assert_eq!(ctx.headers().len(), 4);
    }

    #[test]
    #[allow(deprecated)]
    fn test_rest_context_alias() {
        // The deprecated alias still resolves to `Client`.
        let ctx: RestContext = RestContext::new();
        assert_eq!(ctx.config().host(), "www.atonline.com");
    }
}
