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

/// Context for REST API requests
#[derive(Clone)]
pub struct RestContext {
    /// Configuration
    config: Config,
    /// Optional authentication token (shared so renewals persist across calls)
    token: Arc<Mutex<Option<Token>>>,
    /// Optional API key
    api_key: Option<ApiKey>,
}

impl RestContext {
    /// Create a new REST context with default configuration
    pub fn new() -> Self {
        RestContext {
            config: Config::default(),
            token: Arc::new(Mutex::new(None)),
            api_key: None,
        }
    }

    /// Create a new REST context with custom configuration
    pub fn with_config(config: Config) -> Self {
        RestContext {
            config,
            token: Arc::new(Mutex::new(None)),
            api_key: None,
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

        // Create a context without token to avoid recursion
        let ctx = RestContext {
            config: self.config.clone(),
            token: Arc::new(Mutex::new(None)),
            api_key: None,
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

impl Default for RestContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to create a new REST context and make a request
pub fn apply<T, P>(path: &str, method: &str, param: P) -> Result<T>
where
    T: serde::de::DeserializeOwned,
    P: Serialize,
{
    RestContext::new().apply(path, method, param)
}

/// Convenience function to create a new REST context and execute a request
pub fn do_request<P>(path: &str, method: &str, param: P) -> Result<Response>
where
    P: Serialize,
{
    RestContext::new().do_request(path, method, param)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rest_context_creation() {
        let ctx = RestContext::new();
        assert_eq!(ctx.config().scheme(), "https");
        assert_eq!(ctx.config().host(), "www.atonline.com");
    }

    #[test]
    fn test_rest_context_with_config() {
        let config = Config::new("http".to_string(), "localhost:8080".to_string());
        let ctx = RestContext::with_config(config);
        assert_eq!(ctx.config().scheme(), "http");
        assert_eq!(ctx.config().host(), "localhost:8080");
    }
}
