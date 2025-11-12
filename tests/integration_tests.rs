use klbfw::{RestContext, RestError};
use serde::Deserialize;
use std::collections::HashMap;

#[test]
#[ignore] // Run with: cargo test --test integration_tests -- --ignored
fn test_fixed_array() {
    let ctx = RestContext::new();

    // Using apply to unmarshal into a map
    let result: HashMap<String, serde_json::Value> = ctx
        .apply("Misc/Debug:fixedArray", "GET", serde_json::json!({}))
        .expect("failed to call fixedArray");

    // Verify we got a non-empty result
    assert!(!result.is_empty(), "expected non-empty array, got empty result");

    println!("Fixed array test passed: {:?}", result);
}

#[test]
#[ignore]
fn test_fixed_string() {
    let ctx = RestContext::new();

    let response = ctx
        .do_request("Misc/Debug:fixedString", "GET", serde_json::json!({}))
        .expect("failed to call fixedString");

    // Get the string value using the Response.get_string method
    let str_value = response.get_string("").expect("failed to get string from response");

    assert!(!str_value.is_empty(), "expected non-empty string, got empty string");

    println!("Fixed string test passed: {}", str_value);
}

#[test]
#[ignore]
fn test_error() {
    let ctx = RestContext::new();

    let result = ctx.do_request("Misc/Debug:error", "GET", serde_json::json!({}));

    assert!(result.is_err(), "expected error but got Ok");

    // Verify it's a REST API error
    match result.unwrap_err() {
        RestError::Api { .. } => {
            println!("Error test passed: got API error as expected");
        }
        other => panic!("expected RestError::Api, got {:?}", other),
    }
}

#[test]
#[ignore]
fn test_error_unwrap() {
    let ctx = RestContext::new();

    // Test with the fieldError endpoint
    let result = ctx.do_request(
        "Misc/Debug:fieldError",
        "GET",
        serde_json::json!({"i": 42}),
    );

    assert!(result.is_err(), "expected error but got Ok");

    // Verify it's a REST API error
    match result.unwrap_err() {
        RestError::Api { code, message, .. } => {
            println!("Field error test passed: code={:?}, message={}", code, message);
        }
        other => panic!("expected RestError::Api, got {:?}", other),
    }
}

#[test]
#[ignore]
fn test_redirect() {
    let ctx = RestContext::new();

    let result = ctx.do_request("Misc/Debug:testRedirect", "GET", serde_json::json!({}));

    assert!(result.is_err(), "expected redirect error but got Ok");

    let err = result.unwrap_err();
    let err_msg = err.to_string();

    // Check that we get an error message (redirect handling may vary)
    assert!(!err_msg.is_empty(), "expected error message for redirect");

    println!("Redirect test passed: {}", err_msg);
}

#[test]
#[ignore]
fn test_argument() {
    let ctx = RestContext::new();

    let test_value = "hello world";

    // Test with the required 'input' parameter
    let response = ctx
        .do_request(
            "Misc/Debug:argument",
            "GET",
            serde_json::json!({"input": test_value}),
        )
        .expect("failed to call argument endpoint");

    // The endpoint should return our input value
    let returned_value = response
        .get_string("input")
        .expect("failed to get input value from response");

    assert_eq!(
        returned_value, test_value,
        "expected returned value '{}', got '{}'",
        test_value, returned_value
    );

    println!("Argument test passed: {}", returned_value);
}

#[test]
#[ignore]
fn test_arg_string() {
    let ctx = RestContext::new();

    let test_value = "test string";

    // Using apply to unmarshal directly into a map
    let result: HashMap<String, serde_json::Value> = ctx
        .apply(
            "Misc/Debug:argString",
            "GET",
            serde_json::json!({"input_string": test_value}),
        )
        .expect("failed to call argString endpoint");

    // The endpoint should echo the input_string in the response
    let returned = result
        .get("input_string")
        .and_then(|v| v.as_str())
        .expect("expected input_string in response");

    assert_eq!(
        returned, test_value,
        "expected returned input_string '{}', got '{}'",
        test_value, returned
    );

    println!("Arg string test passed: {}", returned);
}

#[test]
#[ignore]
fn test_response_as() {
    // Define a struct that matches our expected data structure
    #[derive(Debug, Deserialize)]
    struct TestData {
        name: String,
        value: i32,
        items: Vec<String>,
    }

    // Note: This test doesn't make an actual API call
    // It just tests the response parsing functionality
    let json_data = serde_json::json!({
        "name": "test",
        "value": 42,
        "items": ["one", "two", "three"]
    });

    // Simulate a response
    let response = klbfw::Response {
        result: "success".to_string(),
        data: Some(json_data),
        error: None,
        code: None,
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

    // Use apply to unmarshal the data
    let data: TestData = response.apply().expect("apply failed");

    // Verify the data was correctly unmarshaled
    assert_eq!(data.name, "test", "expected Name='test', got '{}'", data.name);
    assert_eq!(data.value, 42, "expected Value=42, got {}", data.value);
    assert_eq!(data.items.len(), 3, "expected 3 items, got {}", data.items.len());
    assert_eq!(data.items[0], "one");
    assert_eq!(data.items[1], "two");
    assert_eq!(data.items[2], "three");

    println!("Response as test passed: {:?}", data);
}
