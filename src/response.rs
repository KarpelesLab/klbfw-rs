use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Param is a convenience type for parameters passed to REST API requests.
pub type Param = std::collections::HashMap<String, Value>;

/// Response represents a REST API response with standard fields.
/// It handles different result types and provides methods to access response data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// "success", "error", or "redirect"
    pub result: String,

    /// Response data payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,

    /// Error message (if result is "error")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Error code (if result is "error")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,

    /// Extra error information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,

    /// Token information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Paging information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paging: Option<Value>,

    /// Job information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<Value>,

    /// Time information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Value>,

    /// Access information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<Value>,

    /// Exception class name (if result is "redirect")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<String>,

    /// Redirect URL (if result is "redirect")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_url: Option<String>,

    /// Redirect HTTP code (if result is "redirect")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_code: Option<i32>,

    /// X-Request-Id header from HTTP response (not serialized)
    #[serde(skip)]
    pub request_id: Option<String>,
}

impl Response {
    /// Get the raw data value from the response
    pub fn raw(&self) -> Option<&Value> {
        self.data.as_ref()
    }

    /// Get the complete response as a map including metadata
    pub fn full_raw(&self) -> serde_json::Map<String, Value> {
        let mut map = serde_json::Map::new();

        map.insert("result".to_string(), Value::String(self.result.clone()));

        if let Some(ref data) = self.data {
            map.insert("data".to_string(), data.clone());
        }

        if let Some(ref error) = self.error {
            map.insert("error".to_string(), Value::String(error.clone()));
        }

        if let Some(code) = self.code {
            map.insert("code".to_string(), Value::Number(code.into()));
        }

        if let Some(ref extra) = self.extra {
            map.insert("extra".to_string(), Value::String(extra.clone()));
        }

        if let Some(ref token) = self.token {
            map.insert("token".to_string(), Value::String(token.clone()));
        }

        if let Some(ref paging) = self.paging {
            map.insert("paging".to_string(), paging.clone());
        }

        if let Some(ref job) = self.job {
            map.insert("job".to_string(), job.clone());
        }

        if let Some(ref time) = self.time {
            map.insert("time".to_string(), time.clone());
        }

        if let Some(ref access) = self.access {
            map.insert("access".to_string(), access.clone());
        }

        if let Some(ref exception) = self.exception {
            map.insert("exception".to_string(), Value::String(exception.clone()));
        }

        if let Some(ref redirect_url) = self.redirect_url {
            map.insert("redirect_url".to_string(), Value::String(redirect_url.clone()));
        }

        if let Some(redirect_code) = self.redirect_code {
            map.insert("redirect_code".to_string(), Value::Number(redirect_code.into()));
        }

        map
    }

    /// Apply unmarshals the response data into the provided type
    pub fn apply<T>(&self) -> Result<T, crate::error::RestError>
    where
        T: serde::de::DeserializeOwned,
    {
        match &self.data {
            Some(data) => serde_json::from_value(data.clone()).map_err(|e| e.into()),
            None => serde_json::from_value(Value::Null).map_err(|e| e.into()),
        }
    }

    /// Get a value from the response data by a slash-separated path.
    /// For example, "user/name" would access the "name" field inside the "user" object.
    pub fn get(&self, path: &str) -> Option<&Value> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        let mut current = self.data.as_ref()?;

        for part in parts {
            current = match current {
                Value::Object(map) => map.get(part)?,
                Value::Array(arr) => {
                    let index: usize = part.parse().ok()?;
                    arr.get(index)?
                }
                _ => return None,
            };
        }

        Some(current)
    }

    /// Get a string value from the response data by a slash-separated path
    pub fn get_string(&self, path: &str) -> Option<String> {
        self.get(path).and_then(|v| v.as_str().map(|s| s.to_string()))
    }

    /// Get metadata fields with @ prefix
    pub fn offset_get(&self, key: &str) -> Option<Value> {
        if let Some(stripped) = key.strip_prefix('@') {
            match stripped {
                "error" => self.error.as_ref().map(|s| Value::String(s.clone())),
                "code" => self.code.map(|c| Value::Number(c.into())),
                "extra" => self.extra.as_ref().map(|s| Value::String(s.clone())),
                "token" => self.token.as_ref().map(|s| Value::String(s.clone())),
                "paging" => self.paging.clone(),
                "job" => self.job.clone(),
                "time" => self.time.clone(),
                "access" => self.access.clone(),
                "exception" => self.exception.as_ref().map(|s| Value::String(s.clone())),
                _ => None,
            }
        } else {
            self.get(key).cloned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "result": "success",
            "data": {"user": {"name": "test"}}
        }"#;

        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.result, "success");
        assert!(response.data.is_some());
    }

    #[test]
    fn test_response_get() {
        let json = r#"{
            "result": "success",
            "data": {"user": {"name": "test"}}
        }"#;

        let response: Response = serde_json::from_str(json).unwrap();
        let name = response.get_string("user/name");
        assert_eq!(name, Some("test".to_string()));
    }

    #[test]
    fn test_response_apply() {
        #[derive(Deserialize)]
        struct User {
            name: String,
        }

        let json = r#"{
            "result": "success",
            "data": {"name": "test"}
        }"#;

        let response: Response = serde_json::from_str(json).unwrap();
        let user: User = response.apply().unwrap();
        assert_eq!(user.name, "test");
    }
}
