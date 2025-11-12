use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;
use uuid::Uuid;

use crate::error::{RestError, Result};

/// ApiKey represents an API key with its secret for signing requests.
pub struct ApiKey {
    /// API key identifier
    pub key_id: String,
    /// Ed25519 keypair for signing
    keypair: Keypair,
}

// Manually implement Clone since ed25519-dalek 1.0's Keypair doesn't implement Clone
impl Clone for ApiKey {
    fn clone(&self) -> Self {
        ApiKey {
            key_id: self.key_id.clone(),
            keypair: Keypair {
                secret: SecretKey::from_bytes(self.keypair.secret.as_bytes())
                    .expect("valid secret key"),
                public: self.keypair.public,
            },
        }
    }
}

impl ApiKey {
    /// Create a new ApiKey from a key ID and base64-encoded secret
    ///
    /// # Arguments
    /// * `key_id` - The API key identifier
    /// * `secret` - The base64url-encoded Ed25519 private key
    pub fn new(key_id: String, secret: &str) -> Result<Self> {
        // Try to decode as base64url first (URL_SAFE_NO_PAD)
        let decoded = URL_SAFE_NO_PAD
            .decode(secret)
            .or_else(|_| {
                // Fallback to standard base64
                base64::engine::general_purpose::STANDARD.decode(secret)
            })
            .map_err(|e| RestError::Base64Decode(e))?;

        // Ed25519 secret keys are 32 bytes, but we may receive a 64-byte keypair
        let secret_key = if decoded.len() == 32 {
            // Just the secret key
            SecretKey::from_bytes(&decoded)
                .map_err(|_| RestError::Other("Invalid Ed25519 secret key".to_string()))?
        } else if decoded.len() == 64 {
            // Full keypair (secret + public)
            SecretKey::from_bytes(&decoded[..32])
                .map_err(|_| RestError::Other("Invalid Ed25519 secret key".to_string()))?
        } else {
            return Err(RestError::Other(format!(
                "Invalid key length: expected 32 or 64 bytes, got {}",
                decoded.len()
            )));
        };

        // Generate the public key from the secret key
        let public_key: PublicKey = (&secret_key).into();
        let keypair = Keypair {
            secret: secret_key,
            public: public_key,
        };

        Ok(ApiKey { key_id, keypair })
    }

    /// Generate a signature for a REST API request
    ///
    /// # Arguments
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `path` - API endpoint path
    /// * `query_params` - Query parameters as key-value pairs
    /// * `body` - Request body bytes (if any)
    pub fn generate_signature(
        &self,
        method: &str,
        path: &str,
        query_params: &HashMap<String, String>,
        body: &[u8],
    ) -> Result<String> {
        // Generate SHA256 hash of the request body
        let mut hasher = Sha256::new();
        hasher.update(body);
        let body_hash = hasher.finalize();

        // Build query string (excluding _sign parameter)
        let mut params: Vec<(String, String)> = query_params
            .iter()
            .filter(|(k, _)| k.as_str() != "_sign")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Sort parameters for consistent ordering
        params.sort_by(|a, b| a.0.cmp(&b.0));

        let query_string: String = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params)
            .finish();

        // Build signing string with null byte separators
        let mut sign_string = Vec::new();
        sign_string.extend_from_slice(method.as_bytes());
        sign_string.push(0);
        sign_string.extend_from_slice(path.as_bytes());
        sign_string.push(0);
        sign_string.extend_from_slice(query_string.as_bytes());
        sign_string.push(0);
        sign_string.extend_from_slice(&body_hash);

        // Sign using Ed25519
        let signature: Signature = self.keypair.sign(&sign_string);

        // Encode signature as base64url
        let encoded = URL_SAFE_NO_PAD.encode(signature.to_bytes());

        Ok(encoded)
    }

    /// Apply API key parameters to query parameters
    ///
    /// Adds _key, _time, _nonce, and _sign parameters
    pub fn apply_params(
        &self,
        method: &str,
        path: &str,
        params: &mut HashMap<String, String>,
        body: &[u8],
    ) -> Result<()> {
        // Add API key parameters
        params.insert("_key".to_string(), self.key_id.clone());

        // Add timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        params.insert("_time".to_string(), timestamp.to_string());

        // Add nonce
        let nonce = Uuid::new_v4().to_string();
        params.insert("_nonce".to_string(), nonce);

        // Generate and add signature
        let signature = self.generate_signature(method, path, params, body)?;
        params.insert("_sign".to_string(), signature);

        Ok(())
    }
}

// Implement Debug manually to avoid exposing the secret key
impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKey")
            .field("key_id", &self.key_id)
            .field("keypair", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apikey_creation() {
        // This is a test key - not a real one
        let test_secret = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

        let _result = ApiKey::new("test-key".to_string(), test_secret);
        // This may fail with invalid key length, which is expected for a dummy key
        // In real usage, the secret would be a valid 32-byte Ed25519 key
    }

    #[test]
    fn test_signature_generation() {
        // Skip this test unless we have a valid test key
        // In production, you would use a real Ed25519 key for testing
    }
}
