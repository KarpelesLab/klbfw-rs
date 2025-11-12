use crate::apikey::ApiKey;
use crate::client::{create_rest_client, Config};
use crate::error::{RestError, Result};
use crate::response::Response;
use crate::token::Token;
use reqwest::blocking::Client;
use reqwest::Method;
use serde::Serialize;
use std::collections::HashMap;
use url::Url;

/// Context for REST API requests
pub struct RestContext {
    /// HTTP client
    pub client: Client,
    /// Configuration
    pub config: Config,
    /// Optional authentication token
    pub token: Option<Token>,
    /// Optional API key
    pub api_key: Option<ApiKey>,
}

impl RestContext {
    /// Create a new REST context with default configuration
    pub fn new() -> Self {
        RestContext {
            client: create_rest_client(),
            config: Config::default(),
            token: None,
            api_key: None,
        }
    }

    /// Create a new REST context with custom configuration
    pub fn with_config(config: Config) -> Self {
        RestContext {
            client: create_rest_client(),
            config,
            token: None,
            api_key: None,
        }
    }

    /// Set the authentication token
    pub fn with_token(mut self, token: Token) -> Self {
        self.token = Some(token);
        self
    }

    /// Set the API key
    pub fn with_api_key(mut self, api_key: ApiKey) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Enable debug mode
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.config.debug = debug;
        self
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
        // Build base URL
        let base_url = self.config.base_url();
        let url = format!("{}/_special/rest/{}", base_url, path);

        // Serialize parameters
        let param_json = serde_json::to_value(param)?;

        // Build request based on method
        let http_method = Method::from_bytes(method.as_bytes())
            .map_err(|_| RestError::RequestBuild(format!("Invalid HTTP method: {}", method)))?;

        let mut query_params: HashMap<String, String> = HashMap::new();
        let mut body_bytes: Vec<u8> = Vec::new();

        match method {
            "GET" | "HEAD" | "OPTIONS" => {
                // Parameters go in query string
                let param_str = serde_json::to_string(&param_json)?;
                query_params.insert("_".to_string(), param_str);
            }
            "PUT" | "POST" | "PATCH" => {
                // Parameters go in request body
                body_bytes = serde_json::to_vec(&param_json)?;
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

        // Build URL with query parameters
        let mut url_parsed = Url::parse(&url)?;
        for (key, value) in &query_params {
            url_parsed.query_pairs_mut().append_pair(key, value);
        }

        // Build HTTP request
        let mut request = self
            .client
            .request(http_method, url_parsed.as_str())
            .header("Sec-Rest-Http", "false");

        // Add authentication header if using token (and not API key)
        if self.api_key.is_none() {
            if let Some(ref token) = self.token {
                request = request.header("Authorization", format!("Bearer {}", token.access_token));
            }
        }

        // Add body for POST/PUT/PATCH
        if !body_bytes.is_empty() {
            request = request
                .header("Content-Type", "application/json")
                .body(body_bytes.clone());
        }

        // Execute request
        let start = std::time::Instant::now();
        let http_response = request.send()?;
        let status = http_response.status();

        // Get X-Request-Id header
        let request_id = http_response
            .headers()
            .get("X-Request-Id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Read response body
        let body = http_response.bytes()?;

        if self.config.debug {
            let duration = start.elapsed();
            eprintln!(
                "[rest] {} {} => {:?} (status: {})",
                method, path, duration, status
            );
        }

        // Parse response
        let mut response: Response = serde_json::from_slice(&body).map_err(|e| {
            if status.is_client_error() || status.is_server_error() {
                RestError::http(
                    status.as_u16(),
                    String::from_utf8_lossy(&body).to_string(),
                    Some(Box::new(e)),
                )
            } else {
                RestError::Json(e)
            }
        })?;

        response.request_id = request_id;

        // Check for token expiration and renew if needed
        if let Some(ref mut token) = self.token.clone() {
            if response.token.as_deref() == Some("invalid_request_token")
                && response.extra.as_deref() == Some("token_expired")
            {
                if self.config.debug {
                    eprintln!("[rest] Token expired, attempting renewal");
                }

                // Attempt to renew token
                self.renew_token(token)?;

                // Retry the request with new token
                let mut retry_ctx = self.clone();
                retry_ctx.token = Some(token.clone());
                return retry_ctx.do_request(path, method, param_json);
            }
        }

        // Check for redirect
        if response.result == "redirect" {
            if response.exception.as_deref() == Some("Exception\\Login") {
                return Err(RestError::LoginRequired);
            }
            // For other redirects, we could return a redirect error
            // but for now we'll treat them as errors
            return Err(RestError::from_response(response));
        }

        // Check for error response
        if response.result == "error" {
            return Err(RestError::from_response(response));
        }

        Ok(response)
    }

    /// Renew an expired token
    fn renew_token(&self, token: &mut Token) -> Result<()> {
        if !token.has_client_id() {
            return Err(RestError::NoClientId);
        }
        if !token.has_refresh_token() {
            return Err(RestError::NoRefreshToken);
        }

        // Create a context without token to avoid recursion
        let ctx = RestContext {
            client: self.client.clone(),
            config: self.config.clone(),
            token: None,
            api_key: None,
        };

        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("client_id", &token.client_id);
        params.insert("refresh_token", &token.refresh_token);
        params.insert("noraw", "true");

        let renewed: Token = ctx.apply("OAuth2:token", "POST", params)?;

        // Update the token
        token.access_token = renewed.access_token;
        token.refresh_token = renewed.refresh_token;
        token.expires_in = renewed.expires_in;

        Ok(())
    }
}

impl Default for RestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for RestContext {
    fn clone(&self) -> Self {
        RestContext {
            client: self.client.clone(),
            config: self.config.clone(),
            token: self.token.clone(),
            api_key: self.api_key.clone(),
        }
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
        assert_eq!(ctx.config.scheme, "https");
        assert_eq!(ctx.config.host, "www.atonline.com");
    }

    #[test]
    fn test_rest_context_with_config() {
        let config = Config::new("http".to_string(), "localhost:8080".to_string());
        let ctx = RestContext::with_config(config);
        assert_eq!(ctx.config.scheme, "http");
        assert_eq!(ctx.config.host, "localhost:8080");
    }
}
