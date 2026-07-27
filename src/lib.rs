//! # klbfw - KarpelesLab REST Framework for Rust
//!
//! A comprehensive Rust client for interacting with RESTful API services.
//! This library simplifies making HTTP requests to REST endpoints, handling
//! authentication, token renewal, and response parsing.
//!
//! ## Features
//!
//! - Simple API for RESTful requests with JSON encoding/decoding
//! - Multiple authentication methods:
//!   - OAuth2 token management with automatic renewal
//!   - API key authentication with secure Ed25519 request signing
//! - Robust error handling with detailed error types
//! - Custom Time type for API timestamp handling
//! - Response parsing with path-based value access
//!
//! ## Basic Usage
//!
//! ```no_run
//! use klbfw::{Client, Response};
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct User {
//!     id: String,
//!     name: String,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client
//!     let ctx = Client::new();
//!
//!     // Make a simple GET request
//!     let user: User = ctx.apply("Users/Get", "GET", serde_json::json!({
//!         "userId": "123"
//!     }))?;
//!
//!     println!("User: {} ({})", user.name, user.id);
//!     Ok(())
//! }
//! ```
//!
//! ## Authentication
//!
//! ### Token Authentication
//!
//! ```no_run
//! use klbfw::{Client, Token};
//!
//! let token = Token::new(
//!     "access_token".to_string(),
//!     "refresh_token".to_string(),
//!     "client_id".to_string(),
//!     3600,
//! );
//!
//! let ctx = Client::new().with_token(token);
//! ```
//!
//! ### API Key Authentication
//!
//! ```no_run
//! use klbfw::{Client, ApiKey};
//!
//! let api_key = ApiKey::new(
//!     "key-12345".to_string(),
//!     "base64_encoded_secret",
//! )?;
//!
//! let ctx = Client::new().with_api_key(api_key);
//! # Ok::<(), klbfw::RestError>(())
//! ```

// The HTTP stack (blocking rsurl, uuid, tempfile, quick-xml, form-urlencoded,
// idna) is native-only — it cannot compile on wasm32. The browser reaches the
// backend through the `spot` feature instead (REST over an authenticated Spot
// connection), which needs only `response` + `error`. So on wasm this crate
// exposes exactly `response`, `error`, and (with the feature) `spot`.
#[cfg(not(target_arch = "wasm32"))]
pub mod apikey;
#[cfg(not(target_arch = "wasm32"))]
pub mod client;
pub mod error;
pub mod response;
#[cfg(not(target_arch = "wasm32"))]
pub mod rest;
#[cfg(feature = "spot")]
pub mod spot;
#[cfg(not(target_arch = "wasm32"))]
pub mod time;
#[cfg(not(target_arch = "wasm32"))]
pub mod token;
#[cfg(not(target_arch = "wasm32"))]
pub mod upload;

// Re-export main types for convenience
#[cfg(not(target_arch = "wasm32"))]
pub use apikey::ApiKey;
#[cfg(not(target_arch = "wasm32"))]
pub use client::Config;
pub use error::{RestError, Result};
pub use response::{Param, Response};
#[cfg(not(target_arch = "wasm32"))]
#[allow(deprecated)]
pub use rest::RestContext;
#[cfg(not(target_arch = "wasm32"))]
pub use rest::{apply, do_request, Client};
#[cfg(feature = "spot")]
pub use spot::{spot_apply, spot_do_request, SpotClient};
#[cfg(not(target_arch = "wasm32"))]
pub use time::Time;
#[cfg(not(target_arch = "wasm32"))]
pub use token::Token;
#[cfg(not(target_arch = "wasm32"))]
pub use upload::{upload, UploadInfo, UploadProgressFn};

// Re-export serde_json for convenience
pub use serde_json::json;
