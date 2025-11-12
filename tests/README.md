# Integration Tests

This directory contains integration tests that use actual API endpoints to verify the functionality of the klbfw-rs library.

## Running Tests

These tests are marked with `#[ignore]` by default because they require network access and hit actual API endpoints.

To run all integration tests:

```bash
cargo test --tests -- --ignored
```

To run specific test suites:

```bash
# Run only REST API tests
cargo test --test integration_tests -- --ignored

# Run only upload tests
cargo test --test upload_tests -- --ignored
```

To run a specific test:

```bash
cargo test --test integration_tests test_fixed_array -- --ignored
```

## Test Categories

### Integration Tests (`integration_tests.rs`)

Tests basic REST API functionality using the `Misc/Debug:*` endpoints:

- **test_fixed_array**: Tests endpoint that returns a fixed array
- **test_fixed_string**: Tests endpoint that returns a fixed string
- **test_error**: Tests error handling with an endpoint that returns errors
- **test_error_unwrap**: Tests error unwrapping and type checking
- **test_redirect**: Tests redirect response handling
- **test_argument**: Tests parameter passing and echoing
- **test_arg_string**: Tests string parameter handling
- **test_response_as**: Tests response deserialization

### Upload Tests (`upload_tests.rs`)

Tests file upload functionality using the `Misc/Debug:testUpload` endpoint:

- **test_upload_standard**: Tests standard multipart upload (16MB)
- **test_upload_put_only**: Tests direct PUT upload (2MB)
- **test_upload_empty**: Tests uploading empty files (standard mode)
- **test_upload_empty_put_only**: Tests uploading empty files (PUT mode)
- **test_upload_65k**: Tests uploading exactly 65536 bytes
- **test_upload_with_progress**: Tests progress tracking callbacks (5MB)

## Test Endpoints

All tests use the `Misc/Debug:*` endpoints which are designed for testing purposes:

- `Misc/Debug:fixedArray` - Returns a fixed array
- `Misc/Debug:fixedString` - Returns a fixed string
- `Misc/Debug:error` - Returns an error response
- `Misc/Debug:fieldError` - Returns a field-specific error
- `Misc/Debug:testRedirect` - Returns a redirect response
- `Misc/Debug:argument` - Echoes input parameter
- `Misc/Debug:argString` - Echoes string parameter
- `Misc/Debug:testUpload` - Handles file uploads with various options

## Notes

- Tests verify both successful operations and error handling
- Upload tests generate random data and verify SHA256 checksums
- Progress tracking is tested with callbacks
- Tests match the behavior of the original Go library tests
