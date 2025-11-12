use crate::response::Response;
use thiserror::Error;

/// Main error type for REST API operations
#[derive(Debug, Error)]
pub enum RestError {
    /// Error returned by REST API endpoint
    #[error("REST API error: {message}")]
    Api {
        message: String,
        code: Option<i32>,
        request_id: Option<String>,
        response: Response,
    },

    /// HTTP transport error
    #[error("HTTP error {status}: {body}")]
    Http {
        status: u16,
        body: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Login required error
    #[error("login required")]
    LoginRequired,

    /// Token renewal errors
    #[error("no client_id provided for token renewal")]
    NoClientId,

    #[error("no refresh token available and access token has expired")]
    NoRefreshToken,

    /// Request building error
    #[error("failed to build request: {0}")]
    RequestBuild(String),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP client error
    #[error("HTTP client error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// URL parsing error
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// Base64 decoding error
    #[error("Base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Other errors
    #[error("{0}")]
    Other(String),
}

impl RestError {
    /// Create a new API error from a Response
    pub fn from_response(response: Response) -> Self {
        let message = response
            .error
            .clone()
            .unwrap_or_else(|| "unknown error".to_string());
        let code = response.code;
        let request_id = response.request_id.clone();

        RestError::Api {
            message,
            code,
            request_id,
            response,
        }
    }

    /// Create a new HTTP error
    pub fn http(status: u16, body: String, source: Option<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        RestError::Http { status, body, source }
    }

    /// Check if this error is a permission denied error (403)
    pub fn is_permission_denied(&self) -> bool {
        matches!(self, RestError::Api { code: Some(403), .. })
    }

    /// Check if this error is a not found error (404)
    pub fn is_not_found(&self) -> bool {
        matches!(self, RestError::Api { code: Some(404), .. })
    }

    /// Get the HTTP status code if this is an API error
    pub fn status_code(&self) -> Option<i32> {
        match self {
            RestError::Api { code, .. } => *code,
            RestError::Http { status, .. } => Some(*status as i32),
            _ => None,
        }
    }
}

/// Result type for REST operations
pub type Result<T> = std::result::Result<T, RestError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_permission_denied() {
        let response = Response {
            result: "error".to_string(),
            data: None,
            error: Some("permission denied".to_string()),
            code: Some(403),
            extra: None,
            token: None,
            paging: None,
            job: None,
            time: None,
            access: None,
            exception: None,
            redirect_url: None,
            redirect_code: None,
            request_id: None,
        };

        let error = RestError::from_response(response);
        assert!(error.is_permission_denied());
    }

    #[test]
    fn test_error_not_found() {
        let response = Response {
            result: "error".to_string(),
            data: None,
            error: Some("not found".to_string()),
            code: Some(404),
            extra: None,
            token: None,
            paging: None,
            job: None,
            time: None,
            access: None,
            exception: None,
            redirect_url: None,
            redirect_code: None,
            request_id: None,
        };

        let error = RestError::from_response(response);
        assert!(error.is_not_found());
    }
}
