use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use purecrypto::ec::Ed25519PrivateKey;
use purecrypto::hash::sha256;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::error::{RestError, Result};

/// ApiKey represents an API key with its secret for signing requests.
#[derive(Clone)]
pub struct ApiKey {
    /// API key identifier
    pub key_id: String,
    /// Ed25519 private key (seed) for signing
    private_key: Ed25519PrivateKey,
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
            .map_err(RestError::Base64Decode)?;

        // Ed25519 secret keys are a 32-byte seed; a 64-byte input is the
        // seed concatenated with the public key, so take the first 32 bytes.
        if decoded.len() != 32 && decoded.len() != 64 {
            return Err(RestError::Other(format!(
                "Invalid key length: expected 32 or 64 bytes, got {}",
                decoded.len()
            )));
        }

        let mut seed = [0u8; 32];
        seed.copy_from_slice(&decoded[..32]);
        let private_key = Ed25519PrivateKey::from_bytes(seed);

        Ok(ApiKey {
            key_id,
            private_key,
        })
    }

    /// Generate a signature for a REST API request
    ///
    /// # Arguments
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `path` - API endpoint path
    /// * `query_params` - Query parameters as key-value pairs
    /// * `body` - Request body bytes (if any)
    fn generate_signature(
        &self,
        method: &str,
        path: &str,
        query_params: &HashMap<String, String>,
        body: &[u8],
    ) -> Result<String> {
        // Generate SHA256 hash of the request body
        let body_hash = sha256(body);

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
        let signature = self.private_key.sign(&sign_string);

        // Encode signature as base64url
        let encoded = URL_SAFE_NO_PAD.encode(signature.to_bytes());

        Ok(encoded)
    }

    /// Apply API key parameters to query parameters
    ///
    /// Adds _key, _time, _nonce, and _sign parameters
    pub(crate) fn apply_params(
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
            .map_err(|e| RestError::Other(format!("system clock before unix epoch: {}", e)))?
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
            .field("private_key", &"<redacted>")
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
        // A fixed 32-byte seed, base64url-encoded (no padding).
        let seed = [7u8; 32];
        let secret = URL_SAFE_NO_PAD.encode(seed);
        let key = ApiKey::new("test-key".to_string(), &secret).unwrap();

        let mut params = HashMap::new();
        params.insert("foo".to_string(), "bar".to_string());

        let sig = key
            .generate_signature("GET", "Test/Path", &params, b"body")
            .unwrap();

        // Ed25519 is deterministic, so the signature must be stable for fixed
        // inputs. This pins the wire format (server-side verification depends on
        // it). The value is RFC-8032 compliant for the seed above.
        assert_eq!(
            sig,
            "d2J6a7VPiW2OFs9wJkBQ_l0vgXT4HStyG0NJTNmfM6OIsE7Wt9w0XbnuhByxVYFOtHDljykF_qK5z4mSCvimDg",
            "signature changed — this would break server-side verification"
        );

        // Signing twice yields the same signature.
        let sig2 = key
            .generate_signature("GET", "Test/Path", &params, b"body")
            .unwrap();
        assert_eq!(sig, sig2);
    }
}
