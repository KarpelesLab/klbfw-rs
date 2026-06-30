use crate::error::{RestError, Result};
use crate::response::Response;
use crate::rest::RestContext;
use purecrypto::hash::sha256;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;

/// Overall request timeout for uploads (1 hour).
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(3600);
/// Connection establishment timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Lowercase-hex encode a byte slice.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Progress callback function type for upload progress tracking
pub type UploadProgressFn = Box<dyn Fn(i64) + Send + Sync>;

/// Upload configuration and state for file uploads.
/// Supports different upload methods: direct PUT, multi-part uploads, and AWS S3 uploads.
pub struct UploadInfo {
    /// PUT URL for upload
    put: String,
    /// Complete endpoint to call after upload
    complete: String,
    /// Context for making API calls
    ctx: RestContext,
    /// Upper bound on the size of a single multipart part, in MB (defaults to
    /// 1024). The part size is otherwise chosen automatically to target ~10000
    /// parts; this caps that value.
    pub max_part_size: i64,
    /// Number of parallel uploads (defaults to 3)
    pub parallel_uploads: usize,
    /// Progress callback
    progress: Option<Arc<UploadProgressFn>>,

    // PUT upload specific
    blocksize: Option<i64>,

    // AWS upload specific
    aws_id: Option<String>,
    aws_key: Option<String>,
    aws_region: Option<String>,
    aws_name: Option<String>,
    aws_host: Option<String>,
    aws_upload_id: Option<String>,
    aws_tags: Arc<Mutex<Vec<String>>>,
}

/// Response structure for AWS multipart upload initialization
#[derive(Debug, Deserialize)]
struct UploadAwsResp {
    #[serde(rename = "Bucket")]
    #[allow(dead_code)]
    bucket: String,
    #[serde(rename = "Key")]
    #[allow(dead_code)]
    key: String,
    #[serde(rename = "UploadId")]
    upload_id: String,
}

/// Authorization response structure
#[derive(Debug, Deserialize)]
struct UploadAuth {
    authorization: String,
}

/// Numeral wait group for managing parallel operations with a maximum count
struct NumeralWaitGroup {
    count: Arc<(Mutex<i32>, Condvar)>,
}

impl NumeralWaitGroup {
    fn new() -> Self {
        NumeralWaitGroup {
            count: Arc::new((Mutex::new(0), Condvar::new())),
        }
    }

    fn add(&self, delta: i32) {
        let (lock, cvar) = &*self.count;
        let mut count = lock.lock().unwrap();
        *count += delta;
        if delta < 0 {
            cvar.notify_all();
        }
    }

    fn done(&self) {
        self.add(-1);
    }

    fn wait(&self, min: i32) {
        let (lock, cvar) = &*self.count;
        let mut count = lock.lock().unwrap();
        while *count > min {
            count = cvar.wait(count).unwrap();
        }
    }
}

/// Upload a file to a REST API endpoint
///
/// # Arguments
/// * `ctx` - REST context for authentication
/// * `path` - API endpoint path
/// * `method` - HTTP method for initial request
/// * `params` - Parameters for initial API request
/// * `reader` - Reader for file content
/// * `mime_type` - MIME type of the file
/// * `progress` - Optional progress callback
pub fn upload<R: Read + Seek>(
    ctx: &RestContext,
    path: &str,
    method: &str,
    mut params: HashMap<String, Value>,
    mut reader: R,
    mime_type: &str,
    progress: Option<UploadProgressFn>,
) -> Result<Response> {
    // Try to determine file size
    let file_size = reader.seek(SeekFrom::End(0)).ok().and_then(|size| {
        reader.seek(SeekFrom::Start(0)).ok()?;
        Some(size as i64)
    });

    // Add size to params if known
    if let Some(size) = file_size {
        params
            .entry("size".to_string())
            .or_insert(Value::Number(size.into()));
    }

    // Make initial API request to get upload info
    let response = ctx.do_request(path, method, params)?;
    let upload_info: HashMap<String, Value> = response.apply()?;

    // Prepare upload
    let mut uploader = UploadInfo::prepare(upload_info, ctx.clone())?;
    if let Some(progress_fn) = progress {
        uploader.set_progress(progress_fn);
    }

    // Perform upload
    uploader.do_upload(&mut reader, mime_type, file_size)
}

impl UploadInfo {
    /// Prepare an upload from server response
    pub fn prepare(req: HashMap<String, Value>, ctx: RestContext) -> Result<Self> {
        let put = req
            .get("PUT")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RestError::Other("Missing PUT parameter".to_string()))?
            .to_string();

        let complete = req
            .get("Complete")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RestError::Other("Missing Complete parameter".to_string()))?
            .to_string();

        let mut uploader = UploadInfo {
            put,
            complete,
            ctx,
            max_part_size: 1024,
            parallel_uploads: 3,
            progress: None,
            blocksize: None,
            aws_id: None,
            aws_key: None,
            aws_region: None,
            aws_name: None,
            aws_host: None,
            aws_upload_id: None,
            aws_tags: Arc::new(Mutex::new(Vec::new())),
        };

        // Check for blocksize (new multipart method)
        if let Some(bs) = req.get("Blocksize").and_then(|v| v.as_f64()) {
            uploader.blocksize = Some(bs as i64);
            return Ok(uploader);
        }

        // Check for AWS S3 parameters
        if let Some(aws_id) = req
            .get("Cloud_Aws_Bucket_Upload__")
            .and_then(|v| v.as_str())
        {
            if let Some(bucket) = req.get("Bucket_Endpoint").and_then(|v| v.as_object()) {
                if let (Some(key), Some(region), Some(name), Some(host)) = (
                    req.get("Key").and_then(|v| v.as_str()),
                    bucket.get("Region").and_then(|v| v.as_str()),
                    bucket.get("Name").and_then(|v| v.as_str()),
                    bucket.get("Host").and_then(|v| v.as_str()),
                ) {
                    uploader.aws_id = Some(aws_id.to_string());
                    uploader.aws_key = Some(key.to_string());
                    uploader.aws_region = Some(region.to_string());
                    uploader.aws_name = Some(name.to_string());
                    uploader.aws_host = Some(host.to_string());
                }
            }
        }

        Ok(uploader)
    }

    /// Set progress callback
    pub fn set_progress(&mut self, progress: UploadProgressFn) {
        self.progress = Some(Arc::new(progress));
    }

    /// Report progress
    fn report_progress(&self, bytes: i64) {
        if let Some(ref progress) = self.progress {
            progress(bytes);
        }
    }

    /// Perform the upload
    pub fn do_upload<R: Read + Seek>(
        &mut self,
        reader: &mut R,
        mime_type: &str,
        file_size: Option<i64>,
    ) -> Result<Response> {
        // Report start
        self.report_progress(0);

        // Choose upload method
        if let Some(blocksize) = self.blocksize {
            self.part_upload(reader, mime_type, blocksize)
        } else if self.aws_id.is_some() {
            if file_size.is_none() || file_size.unwrap() > 64 * 1024 * 1024 {
                self.aws_upload(reader, mime_type, file_size)
            } else {
                self.put_upload(reader, mime_type, file_size)
            }
        } else {
            self.put_upload(reader, mime_type, file_size)
        }
    }

    /// Simple PUT upload for small files
    fn put_upload<R: Read>(
        &self,
        reader: &mut R,
        mime_type: &str,
        file_size: Option<i64>,
    ) -> Result<Response> {
        let size = file_size
            .ok_or_else(|| RestError::Other("File size required for PUT upload".to_string()))?;

        if size > 5 * 1024 * 1024 * 1024 {
            return Err(RestError::Other(
                "File too large for PUT upload (>5GB)".to_string(),
            ));
        }

        // Read entire file into memory
        let mut buffer = Vec::with_capacity(size as usize);
        reader.read_to_end(&mut buffer)?;

        // Perform PUT request
        let response = rsurl::Request::new("PUT", &self.put)?
            .header("Content-Type", mime_type)
            .max_time(UPLOAD_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .body(buffer)
            .send()?;

        if !(200..300).contains(&response.status) {
            return Err(RestError::http(
                response.status,
                format!("PUT upload failed with status {}", response.status),
                None,
            ));
        }

        // Report progress
        self.report_progress(size);

        // Complete upload
        self.complete()
    }

    /// Multipart upload using blocksize
    fn part_upload<R: Read>(
        &mut self,
        reader: &mut R,
        mime_type: &str,
        blocksize: i64,
    ) -> Result<Response> {
        let nwg = NumeralWaitGroup::new();
        let mut part_no = 0;

        loop {
            nwg.wait((self.parallel_uploads - 1) as i32);
            part_no += 1;

            // Create temp file for this part
            let mut temp_file = NamedTempFile::new()?;
            let mut copied = 0i64;
            let mut buffer = vec![0u8; 8192];

            // Read blocksize bytes into temp file
            while copied < blocksize {
                let to_read = std::cmp::min(buffer.len() as i64, blocksize - copied) as usize;
                match reader.read(&mut buffer[..to_read])? {
                    0 => break,
                    n => {
                        temp_file.write_all(&buffer[..n])?;
                        copied += n as i64;
                    }
                }
            }

            if copied == 0 {
                break;
            }

            // Upload this part
            let nwg_clone = NumeralWaitGroup {
                count: Arc::clone(&nwg.count),
            };
            nwg.add(1);

            self.upload_part(temp_file, mime_type, part_no, copied, blocksize, nwg_clone)?;

            if copied < blocksize {
                break; // EOF
            }
        }

        nwg.wait(0);
        self.complete()
    }

    /// Upload a single part
    fn upload_part(
        &self,
        temp_file: NamedTempFile,
        mime_type: &str,
        part_no: i32,
        size: i64,
        blocksize: i64,
        nwg: NumeralWaitGroup,
    ) -> Result<()> {
        let mut file = temp_file.reopen()?;
        file.seek(SeekFrom::Start(0))?;

        let start = (part_no - 1) as i64 * blocksize;
        let end = start + size - 1;

        let mut buffer = Vec::with_capacity(size as usize);
        file.read_to_end(&mut buffer)?;

        let response = rsurl::Request::new("PUT", &self.put)?
            .header("Content-Type", mime_type)
            .header("Content-Range", &format!("bytes {}-{}/*", start, end))
            .max_time(UPLOAD_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .body(buffer)
            .send()?;

        if !(200..300).contains(&response.status) {
            nwg.done();
            return Err(RestError::http(
                response.status,
                format!("Part upload failed with status {}", response.status),
                None,
            ));
        }

        self.report_progress(size);
        nwg.done();
        Ok(())
    }

    /// AWS S3 multipart upload for large files
    fn aws_upload<R: Read>(
        &mut self,
        reader: &mut R,
        mime_type: &str,
        file_size: Option<i64>,
    ) -> Result<Response> {
        // Choose the part size in bytes: aim for ~10000 parts with a 5 MiB floor
        // (S3's multipart minimum). When the size is unknown (streaming), fall
        // back to 526 MiB, which stays under 10000 parts up to ~5 TB. This
        // matches the reference JS client; the previous MB-rounded formula could
        // overshoot S3's 10000-part limit for some sizes. The auto value is then
        // capped by the caller-configurable `max_part_size` (kept at or above the
        // 5 MiB floor so the clamp range stays valid).
        let cap = self
            .max_part_size
            .saturating_mul(1024 * 1024)
            .max(5 * 1024 * 1024);
        let block_size: i64 = match file_size {
            Some(size) => {
                if size > 5 * 1024 * 1024 * 1024 * 1024 {
                    return Err(RestError::Other(
                        "File exceeds AWS S3 5TB limit".to_string(),
                    ));
                }
                // ceil(size / 10000); size is non-negative here.
                ((size + 9999) / 10000).clamp(5 * 1024 * 1024, cap)
            }
            None => 551550976.min(cap),
        };

        // Initialize AWS multipart upload
        self.aws_init(mime_type)?;

        let nwg = NumeralWaitGroup::new();
        let mut part_no = 0;

        loop {
            nwg.wait((self.parallel_uploads - 1) as i32);
            part_no += 1;

            // Create temp file for this part
            let mut temp_file = NamedTempFile::new()?;
            let max_bytes = block_size;
            let mut copied = 0i64;
            let mut buffer = vec![0u8; 8192];

            // Read max_bytes into temp file
            while copied < max_bytes {
                let to_read = std::cmp::min(buffer.len() as i64, max_bytes - copied) as usize;
                match reader.read(&mut buffer[..to_read])? {
                    0 => break,
                    n => {
                        temp_file.write_all(&buffer[..n])?;
                        copied += n as i64;
                    }
                }
            }

            if copied == 0 && part_no != 1 {
                break;
            }

            // Upload this part to AWS
            let nwg_clone = NumeralWaitGroup {
                count: Arc::clone(&nwg.count),
            };
            nwg.add(1);

            self.aws_upload_part(temp_file, part_no, copied, nwg_clone)?;

            if copied < max_bytes {
                break; // EOF
            }
        }

        nwg.wait(0);

        // Finalize AWS upload
        self.aws_finalize()?;

        // Trigger the server-side completion handler. The AWS multipart path
        // uses a dedicated endpoint rather than the generic Complete URL.
        let aws_id = self
            .aws_id
            .as_ref()
            .ok_or_else(|| RestError::Other("AWS upload not initialized".to_string()))?;
        self.ctx.do_request(
            &format!("Cloud/Aws/Bucket/Upload/{}:handleComplete", aws_id),
            "POST",
            HashMap::<String, Value>::new(),
        )
    }

    /// Upload a single part to AWS S3
    fn aws_upload_part(
        &self,
        temp_file: NamedTempFile,
        part_no: i32,
        size: i64,
        nwg: NumeralWaitGroup,
    ) -> Result<()> {
        let mut file = temp_file.reopen()?;
        file.seek(SeekFrom::Start(0))?;

        let upload_id = self
            .aws_upload_id
            .as_ref()
            .ok_or_else(|| RestError::Other("AWS upload not initialized".to_string()))?;

        let query = format!("partNumber={}&uploadId={}", part_no, upload_id);
        let response = self.aws_request("PUT", &query, &mut file, None)?;

        // Get ETag from response
        let etag = response
            .header("ETag")
            .ok_or_else(|| RestError::Other("Missing ETag in AWS response".to_string()))?
            .to_string();

        // Store ETag
        self.set_tag(part_no, etag);

        self.report_progress(size);
        nwg.done();
        Ok(())
    }

    /// Store ETag for a part
    fn set_tag(&self, part_no: i32, tag: String) {
        let mut tags = self.aws_tags.lock().unwrap();
        let pos = (part_no - 1) as usize;

        while tags.len() <= pos {
            tags.push(String::new());
        }
        tags[pos] = tag;
    }

    /// Initialize AWS multipart upload
    fn aws_init(&mut self, mime_type: &str) -> Result<()> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), mime_type.to_string());
        headers.insert("X-Amz-Acl".to_string(), "private".to_string());

        let response = self.aws_request("POST", "uploads=", &mut io::empty(), Some(headers))?;

        let body = response.text()?;
        let aws_resp: UploadAwsResp = quick_xml::de::from_str(&body)
            .map_err(|e| RestError::Other(format!("Failed to parse AWS response: {}", e)))?;

        self.aws_upload_id = Some(aws_resp.upload_id);
        Ok(())
    }

    /// Finalize AWS multipart upload
    fn aws_finalize(&self) -> Result<()> {
        let tags = self.aws_tags.lock().unwrap();

        let mut xml = String::from("<CompleteMultipartUpload>");
        for (n, tag) in tags.iter().enumerate() {
            xml.push_str(&format!(
                "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                n + 1,
                tag
            ));
        }
        xml.push_str("</CompleteMultipartUpload>");

        let upload_id = self
            .aws_upload_id
            .as_ref()
            .ok_or_else(|| RestError::Other("AWS upload not initialized".to_string()))?;

        let query = format!("uploadId={}", upload_id);
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/xml".to_string());

        let mut cursor = io::Cursor::new(xml.as_bytes());
        let response = self.aws_request("POST", &query, &mut cursor, Some(headers))?;

        // Read response to ensure completion
        let _ = response.text()?;
        Ok(())
    }

    /// Make an AWS S3 request with signature
    fn aws_request<R: Read + Seek>(
        &self,
        method: &str,
        query: &str,
        body: &mut R,
        headers: Option<HashMap<String, String>>,
    ) -> Result<rsurl::Response> {
        let mut headers = headers.unwrap_or_default();

        // Read the body into a buffer once; reuse it for hashing and sending.
        body.seek(SeekFrom::Start(0))?;
        let mut buffer = Vec::new();
        body.read_to_end(&mut buffer)?;

        // Calculate body hash (sha256 of the empty input is the well-known
        // e3b0c4... digest, so the empty case needs no special handling).
        let body_hash = hex(&sha256(&buffer));

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| RestError::Other(format!("system clock before unix epoch: {}", e)))?;
        let timestamp = chrono::DateTime::from_timestamp(now.as_secs() as i64, 0)
            .ok_or_else(|| RestError::Other("invalid system timestamp".to_string()))?
            .format("%Y%m%dT%H%M%SZ")
            .to_string();
        let date = &timestamp[..8];

        headers.insert("X-Amz-Content-Sha256".to_string(), body_hash.clone());
        headers.insert("X-Amz-Date".to_string(), timestamp.clone());

        let aws_key = self.aws_key.as_ref().unwrap();
        let aws_name = self.aws_name.as_ref().unwrap();
        let aws_host = self.aws_host.as_ref().unwrap();
        let aws_region = self.aws_region.as_ref().unwrap();
        let aws_id = self.aws_id.as_ref().unwrap();

        // Build the string-to-sign for the server's signV4 endpoint. The server
        // reconstructs the AWS SigV4 canonical request from these newline-joined
        // lines, so every signed-header line, the signed-headers list, and the
        // trailing payload hash must be present — otherwise AWS rejects the
        // signature with HTTP 400. This mirrors the reference JS client.
        let mut auth_parts = vec![
            "AWS4-HMAC-SHA256".to_string(),
            timestamp.clone(),
            format!("{}/{}/s3/aws4_request", date, aws_region),
            method.to_string(),
            format!("/{}/{}", aws_name, aws_key),
            query.to_string(),
            format!("host:{}", aws_host),
        ];

        // Sign "host" plus every x-* header, ordered by header name.
        let mut signed_headers = vec!["host".to_string()];
        let mut header_keys: Vec<&String> = headers.keys().collect();
        header_keys.sort();
        for key in header_keys {
            let lower = key.to_lowercase();
            if lower.starts_with("x-") {
                auth_parts.push(format!("{}:{}", lower, headers[key]));
                signed_headers.push(lower);
            }
        }

        auth_parts.push(String::new());
        auth_parts.push(signed_headers.join(";"));
        auth_parts.push(body_hash.clone());

        // Get signature from API
        let auth_str = auth_parts.join("\n");
        let mut params = HashMap::new();
        params.insert("headers".to_string(), Value::String(auth_str));

        let auth_response = self.ctx.do_request(
            &format!("Cloud/Aws/Bucket/Upload/{}:signV4", aws_id),
            "POST",
            params,
        )?;
        let auth: UploadAuth = auth_response.apply()?;

        headers.insert("Authorization".to_string(), auth.authorization);

        // Build URL
        let url = format!("https://{}/{}/{}?{}", aws_host, aws_name, aws_key, query);

        // Make request
        let mut request = rsurl::Request::new(method, &url)?
            .max_time(UPLOAD_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT);
        for (k, v) in &headers {
            request = request.header(k, v);
        }
        let response = request.body(buffer).send()?;

        if !(200..300).contains(&response.status) {
            return Err(RestError::http(
                response.status,
                format!("AWS request failed with status {}", response.status),
                None,
            ));
        }

        Ok(response)
    }

    /// Complete the upload by calling the complete endpoint
    fn complete(&self) -> Result<Response> {
        self.ctx
            .do_request(&self.complete, "POST", HashMap::<String, Value>::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeral_wait_group() {
        let nwg = NumeralWaitGroup::new();
        nwg.add(5);
        nwg.done();
        nwg.done();
        nwg.wait(3);
        // Should not block since count is 3
    }
}
