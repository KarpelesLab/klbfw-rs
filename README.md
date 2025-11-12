# klbfw-rs

A Rust implementation of the KarpelesLab REST Framework client library.

This library provides a comprehensive client for interacting with RESTful API services, featuring authentication, token renewal, and response parsing.

## Features

- **Simple API**: Easy-to-use methods for RESTful requests with JSON encoding/decoding
- **Multiple Authentication Methods**:
  - OAuth2 token management with automatic renewal
  - API key authentication with secure Ed25519 request signing
- **Robust Error Handling**: Detailed error types with conversion to Rust standard error types
- **Custom Time Type**: Handles API timestamps with microsecond precision
- **Response Parsing**: Path-based value access in responses
- **Blocking HTTP Client**: Built on reqwest with connection pooling and timeouts

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
klbfw = "0.1"
```

## Usage

### Basic Request

```rust
use klbfw::RestContext;
use serde::Deserialize;

#[derive(Deserialize)]
struct User {
    id: String,
    name: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a REST context
    let ctx = RestContext::new();

    // Make a simple GET request
    let user: User = ctx.apply("Users/Get", "GET", serde_json::json!({
        "userId": "123"
    }))?;

    println!("User: {} ({})", user.name, user.id);
    Ok(())
}
```

### Token Authentication

```rust
use klbfw::{RestContext, Token};

let token = Token::new(
    "access_token".to_string(),
    "refresh_token".to_string(),
    "client_id".to_string(),
    3600,
);

let ctx = RestContext::new().with_token(token);

// Make authenticated requests
let response = ctx.do_request("Protected/Resource", "GET", serde_json::json!({}))?;
```

### API Key Authentication

```rust
use klbfw::{RestContext, ApiKey};

let api_key = ApiKey::new(
    "key-12345".to_string(),
    "base64_encoded_secret",
)?;

let ctx = RestContext::new().with_api_key(api_key);

// Requests are automatically signed
let response = ctx.do_request("Protected/Resource", "GET", serde_json::json!({}))?;
```

### Custom Configuration

```rust
use klbfw::{RestContext, Config};

let config = Config::new(
    "https".to_string(),
    "api.example.com".to_string(),
).with_debug(true);

let ctx = RestContext::with_config(config);
```

## Implemented Features

Based on the Go version (~/projects/rest):

### Core API Methods
- ✅ `apply()` - Makes REST API request and unmarshals response
- ✅ `do_request()` - Executes request and returns raw Response

### Type System
- ✅ `Time` - Custom time type with JSON serialization matching API format
- ✅ `Response` - REST API response with standard fields
- ✅ `Param` - Convenience type for parameters
- ✅ `RestError` - Comprehensive error type with conversion to std errors

### Authentication
- ✅ `Token` - OAuth2 token with automatic renewal
- ✅ `ApiKey` - API key with Ed25519 signing

### HTTP Client
- ✅ Configurable HTTP client with connection pooling
- ✅ Request/response handling
- ✅ Error parsing and handling

## Differences from Go Version

1. **Blocking vs Async**: This Rust version currently implements a blocking client using `reqwest::blocking`. The Go version uses standard `http.Client` which is also blocking.

2. **Context**: Instead of Go's `context.Context`, this version uses `RestContext` which holds the client configuration and authentication.

3. **Error Handling**: Uses Rust's `Result` type and `thiserror` for error handling, with conversions to standard error types.

4. **Generics**: The Rust version uses generics for type-safe response parsing, similar to the Go version's generic `As` function.

## Testing

Run tests with:

```bash
cargo test
```

All tests pass successfully (14 tests).

## License

MIT

## Port Notes

This is a direct port of the Go library from `~/projects/rest`. Key features implemented:
- Base API access methods (`apply`, `do_request`)
- Time type with custom JSON serialization/deserialization
- Response type with all standard fields
- Error types matching Go implementation
- Token and API key authentication
- HTTP client configuration

The implementation maintains compatibility with the existing API while following Rust idioms and best practices.
