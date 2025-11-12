use klbfw::{upload, RestContext};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Cursor;

/// Generate test data of specified size
fn generate_test_data(size: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut data = vec![0u8; size];
    rand::thread_rng().fill_bytes(&mut data);
    data
}

/// Calculate SHA256 hash of data
fn calculate_sha256(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    format!("{:x}", hash)
}

#[test]
#[ignore] // Run with: cargo test --test upload_tests -- --ignored
fn test_upload_standard() {
    let ctx = RestContext::new();

    // Generate 16MB of test data
    let data = generate_test_data(16 * 1024 * 1024);
    let expected_hash = calculate_sha256(&data);

    println!("Testing standard upload (16MB)...");
    println!("Expected hash: {}", expected_hash);

    let mut params = HashMap::new();
    params.insert("filename".to_string(), serde_json::json!("test.bin"));
    params.insert("put_only".to_string(), serde_json::json!(false));

    let reader = Cursor::new(data);

    let response = upload(
        &ctx,
        "Misc/Debug:testUpload",
        "POST",
        params,
        reader,
        "application/octet-stream",
        Some(Box::new(|bytes| {
            println!("Progress: {} bytes uploaded", bytes);
        })),
    )
    .expect("failed to do standard upload");

    // Verify we got a Blob__ field in the response
    let blob_value = response
        .get_string("Blob__")
        .expect("expected Blob__ field in response");

    assert!(!blob_value.is_empty(), "Blob__ field should not be empty");

    println!("Standard upload test passed!");
    println!("Response: {:?}", response.raw());
}

#[test]
#[ignore]
fn test_upload_put_only() {
    let ctx = RestContext::new();

    // Generate 2MB of test data
    let data = generate_test_data(2 * 1024 * 1024);
    let expected_hash = calculate_sha256(&data);

    println!("Testing PUT-only upload (2MB)...");
    println!("Expected hash: {}", expected_hash);

    let mut params = HashMap::new();
    params.insert("filename".to_string(), serde_json::json!("test_put_only.bin"));
    params.insert("put_only".to_string(), serde_json::json!(true));

    let reader = Cursor::new(data);

    let response = upload(
        &ctx,
        "Misc/Debug:testUpload",
        "POST",
        params,
        reader,
        "application/octet-stream",
        None,
    )
    .expect("failed to do PUT-only upload");

    // Verify we got a Blob__ field in the response
    let blob_value = response
        .get_string("Blob__")
        .expect("expected Blob__ field in response");

    assert!(!blob_value.is_empty(), "Blob__ field should not be empty");

    // Verify SHA256 matches
    if let Some(sha_value) = response.get_string("SHA256") {
        assert_eq!(
            sha_value, expected_hash,
            "SHA256 mismatch: expected {}, got {}",
            expected_hash, sha_value
        );
        println!("SHA256 verified: {}", sha_value);
    }

    println!("PUT-only upload test passed!");
}

#[test]
#[ignore]
fn test_upload_empty() {
    let ctx = RestContext::new();

    println!("Testing empty file upload...");

    let data = Vec::new();
    let expected_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    let mut params = HashMap::new();
    params.insert("filename".to_string(), serde_json::json!("empty.bin"));
    params.insert("put_only".to_string(), serde_json::json!(false));

    let reader = Cursor::new(data);

    let response = upload(
        &ctx,
        "Misc/Debug:testUpload",
        "POST",
        params,
        reader,
        "application/octet-stream",
        None,
    )
    .expect("failed to do empty standard upload");

    // Verify the SHA256 of an empty file
    if let Some(sha_value) = response.get_string("SHA256") {
        assert_eq!(
            sha_value, expected_hash,
            "Expected SHA256 of empty file to be {}, got {}",
            expected_hash, sha_value
        );
        println!("SHA256 verified: {}", sha_value);
    }

    println!("Empty upload test passed!");
}

#[test]
#[ignore]
fn test_upload_empty_put_only() {
    let ctx = RestContext::new();

    println!("Testing empty file upload (PUT-only)...");

    let data = Vec::new();
    let expected_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    let mut params = HashMap::new();
    params.insert("filename".to_string(), serde_json::json!("empty_put_only.bin"));
    params.insert("put_only".to_string(), serde_json::json!(true));

    let reader = Cursor::new(data);

    let response = upload(
        &ctx,
        "Misc/Debug:testUpload",
        "POST",
        params,
        reader,
        "application/octet-stream",
        None,
    )
    .expect("failed to do empty PUT-only upload");

    // Verify the SHA256 of an empty file
    if let Some(sha_value) = response.get_string("SHA256") {
        assert_eq!(
            sha_value, expected_hash,
            "Expected SHA256 of empty file to be {}, got {}",
            expected_hash, sha_value
        );
        println!("SHA256 verified: {}", sha_value);
    }

    println!("Empty PUT-only upload test passed!");
}

#[test]
#[ignore]
fn test_upload_65k() {
    let ctx = RestContext::new();

    // Generate exactly 65536 bytes of random data
    let data = generate_test_data(65536);
    let expected_hash = calculate_sha256(&data);

    println!("Testing 65K upload...");
    println!("Expected hash: {}", expected_hash);

    let mut params = HashMap::new();
    params.insert("filename".to_string(), serde_json::json!("test_65k.bin"));

    let reader = Cursor::new(data);

    let response = upload(
        &ctx,
        "Misc/Debug:testUpload",
        "POST",
        params,
        reader,
        "application/octet-stream",
        None,
    )
    .expect("failed to do 65K upload");

    // Verify the SHA256 from the response
    if let Some(sha_value) = response.get_string("SHA256") {
        assert_eq!(
            sha_value, expected_hash,
            "Expected SHA256 {}, got {}",
            expected_hash, sha_value
        );
        println!("SHA256 verified: {}", sha_value);
    }

    // Verify we got a Blob__ field in the response
    let blob_value = response
        .get_string("Blob__")
        .expect("expected Blob__ field in response");

    assert!(!blob_value.is_empty(), "Blob__ field should not be empty");

    println!("65K upload test passed!");
}

#[test]
#[ignore]
fn test_upload_with_progress() {
    let ctx = RestContext::new();

    // Generate 5MB of test data
    let data = generate_test_data(5 * 1024 * 1024);

    println!("Testing upload with progress tracking (5MB)...");

    let mut params = HashMap::new();
    params.insert("filename".to_string(), serde_json::json!("test_progress.bin"));

    let reader = Cursor::new(data);

    use std::sync::{Arc, Mutex};
    let total_uploaded = Arc::new(Mutex::new(0i64));
    let total_clone = Arc::clone(&total_uploaded);

    let response = upload(
        &ctx,
        "Misc/Debug:testUpload",
        "POST",
        params,
        reader,
        "application/octet-stream",
        Some(Box::new(move |bytes| {
            let mut total = total_clone.lock().unwrap();
            *total += bytes;
            println!("Progress callback: +{} bytes (total: {})", bytes, *total);
        })),
    )
    .expect("failed to do upload with progress");

    let final_total = *total_uploaded.lock().unwrap();
    println!("Total uploaded: {} bytes", final_total);
    assert!(final_total > 0, "Progress callback should have been called");

    // Verify we got a Blob__ field
    let blob_value = response
        .get_string("Blob__")
        .expect("expected Blob__ field in response");

    assert!(!blob_value.is_empty(), "Blob__ field should not be empty");

    println!("Upload with progress test passed!");
}
