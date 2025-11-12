use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::Deref;

/// Custom time type that wraps chrono::DateTime and provides custom JSON serialization
/// matching the format expected by the REST API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time(pub DateTime<Utc>);

/// Internal structure for JSON serialization/deserialization
#[derive(Debug, Serialize, Deserialize)]
struct TimeInternal {
    /// Unix timestamp in seconds
    unix: i64,
    /// Microseconds component
    us: i64,
    /// Timezone (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    tz: Option<String>,
    /// ISO 8601 formatted string (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    iso: Option<String>,
    /// Full timestamp in microseconds as string (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    full: Option<String>,
    /// Unix timestamp in milliseconds as string (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    unixms: Option<String>,
}

impl Time {
    /// Create a new Time from a DateTime
    pub fn new(dt: DateTime<Utc>) -> Self {
        Time(dt)
    }

    /// Create a Time from unix timestamp and microseconds
    pub fn from_unix(unix: i64, usec: i64) -> Self {
        let nanos = (usec * 1000) as u32;
        Time(Utc.timestamp_opt(unix, nanos).unwrap())
    }

    /// Get the unix timestamp in seconds
    pub fn unix(&self) -> i64 {
        self.0.timestamp()
    }

    /// Get the microseconds component
    pub fn usec(&self) -> i64 {
        (self.0.timestamp_subsec_nanos() / 1000) as i64
    }

    /// Get the timestamp in microseconds
    pub fn unix_micro(&self) -> i64 {
        self.0.timestamp_micros()
    }

    /// Get the timestamp in milliseconds
    pub fn unix_milli(&self) -> i64 {
        self.0.timestamp_millis()
    }

    /// Get ISO 8601 formatted string
    pub fn iso(&self) -> String {
        self.0.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

impl Deref for Time {
    type Target = DateTime<Utc>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<DateTime<Utc>> for Time {
    fn from(dt: DateTime<Utc>) -> Self {
        Time(dt)
    }
}

impl From<Time> for DateTime<Utc> {
    fn from(t: Time) -> Self {
        t.0
    }
}

impl Serialize for Time {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let internal = TimeInternal {
            unix: self.unix(),
            us: self.usec(),
            tz: Some("UTC".to_string()),
            iso: Some(self.iso()),
            full: Some(self.unix_micro().to_string()),
            unixms: Some(self.unix_milli().to_string()),
        };
        internal.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Time {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let internal = TimeInternal::deserialize(deserializer)?;
        Ok(Time::from_unix(internal.unix, internal.us))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_time_serialization() {
        let time = Time::from_unix(1597242491, 747497);
        let json = serde_json::to_string(&time).unwrap();

        // Parse back to check structure
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["unix"], 1597242491);
        assert_eq!(value["us"], 747497);
    }

    #[test]
    fn test_time_deserialization() {
        let json = r#"{"unix": 1597242491, "us": 747497}"#;
        let time: Time = serde_json::from_str(json).unwrap();

        assert_eq!(time.unix(), 1597242491);
        assert_eq!(time.usec(), 747497);
    }

    #[test]
    fn test_time_null() {
        let json = "null";
        let result: Result<Option<Time>, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
