use serde::{Deserialize, Serialize};

/// Token represents an OAuth2 token with refresh capabilities.
/// It contains both access and refresh tokens for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// Access token for API requests
    #[serde(rename = "access_token")]
    pub access_token: String,

    /// Refresh token for renewing expired access tokens
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,

    /// Token type (usually "Bearer")
    #[serde(rename = "token_type")]
    pub token_type: String,

    /// Client ID for token renewal
    #[serde(skip)]
    pub client_id: String,

    /// Token expiration time in seconds
    #[serde(rename = "expires_in")]
    pub expires_in: i32,
}

impl Token {
    /// Create a new Token
    pub fn new(
        access_token: String,
        refresh_token: String,
        client_id: String,
        expires_in: i32,
    ) -> Self {
        Token {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            client_id,
            expires_in,
        }
    }

    /// Check if we have a refresh token available
    pub fn has_refresh_token(&self) -> bool {
        !self.refresh_token.is_empty()
    }

    /// Check if we have a client ID available
    pub fn has_client_id(&self) -> bool {
        !self.client_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_creation() {
        let token = Token::new(
            "access123".to_string(),
            "refresh456".to_string(),
            "client789".to_string(),
            3600,
        );

        assert_eq!(token.access_token, "access123");
        assert_eq!(token.refresh_token, "refresh456");
        assert_eq!(token.client_id, "client789");
        assert_eq!(token.expires_in, 3600);
        assert!(token.has_refresh_token());
        assert!(token.has_client_id());
    }

    #[test]
    fn test_token_serialization() {
        let token = Token::new(
            "access123".to_string(),
            "refresh456".to_string(),
            "client789".to_string(),
            3600,
        );

        let json = serde_json::to_string(&token).unwrap();
        assert!(json.contains("access_token"));
        assert!(json.contains("refresh_token"));
    }
}
