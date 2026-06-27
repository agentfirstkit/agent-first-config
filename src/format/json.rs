//! JSON format backend (via serde_json).

use crate::{ConfigError, ConfigResult, Value};

pub fn load(content: &str) -> ConfigResult<Value> {
    serde_json::from_str::<serde_json::Value>(content)
        .map(Value::from)
        .map_err(|e| ConfigError::ParseError {
            format: "JSON".to_string(),
            detail: e.to_string(),
        })
}

pub fn save(value: &Value) -> ConfigResult<String> {
    let json_val: serde_json::Value = value.clone().into();
    serde_json::to_string_pretty(&json_val).map_err(|e| ConfigError::ParseError {
        format: "JSON".to_string(),
        detail: e.to_string(),
    })
}
