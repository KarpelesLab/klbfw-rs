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
//! use klbfw::{RestContext, Response};
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct User {
//!     id: String,
//!     name: String,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a REST context
//!     let ctx = RestContext::new();
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
//! use klbfw::{RestContext, Token};
//!
//! let token = Token::new(
//!     "access_token".to_string(),
//!     "refresh_token".to_string(),
//!     "client_id".to_string(),
//!     3600,
//! );
//!
//! let ctx = RestContext::new().with_token(token);
//! ```
//!
//! ### API Key Authentication
//!
//! ```no_run
//! use klbfw::{RestContext, ApiKey};
//!
//! let api_key = ApiKey::new(
//!     "key-12345".to_string(),
//!     "base64_encoded_secret",
//! )?;
//!
//! let ctx = RestContext::new().with_api_key(api_key);
//! # Ok::<(), klbfw::RestError>(())
//! ```

pub mod apikey;
pub mod client;
pub mod error;
pub mod response;
pub mod rest;
pub mod time;
pub mod token;
pub mod upload;

// Re-export main types for convenience
pub use apikey::ApiKey;
pub use client::Config;
pub use error::{RestError, Result};
pub use response::{Param, Response};
pub use rest::{apply, do_request, RestContext};
pub use time::Time;
pub use token::Token;
pub use upload::{upload, UploadInfo, UploadProgressFn};

// Re-export serde_json for convenience
pub use serde_json::json;
