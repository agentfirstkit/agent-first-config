//! TOML format backend (format-preserving via toml_edit).

use crate::{ConfigError, ConfigResult, Value};

pub fn load(content: &str) -> ConfigResult<Value> {
    toml::from_str::<toml::Value>(content)
        .map(value_to_our_value)
        .map_err(|e| ConfigError::ParseError {
            format: "TOML".to_string(),
            detail: e.to_string(),
        })
}

pub fn save(value: &Value) -> ConfigResult<String> {
    let toml_val = our_value_to_toml_value(value)?;
    toml::to_string_pretty(&toml_val).map_err(|e| ConfigError::ParseError {
        format: "TOML".to_string(),
        detail: e.to_string(),
    })
}

fn value_to_our_value(v: toml::Value) -> Value {
    match v {
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Integer(i) => Value::Integer(i),
        toml::Value::Float(f) => Value::Float(f),
        toml::Value::String(s) => Value::String(s),
        toml::Value::Array(a) => Value::Array(a.into_iter().map(value_to_our_value).collect()),
        toml::Value::Table(t) => {
            let map = t
                .into_iter()
                .map(|(k, v)| (k, value_to_our_value(v)))
                .collect();
            Value::Object(map)
        }
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
    }
}

fn our_value_to_toml_value(v: &Value) -> ConfigResult<toml::Value> {
    match v {
        Value::Null => Ok(toml::Value::String("null".to_string())),
        Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        Value::Integer(i) => Ok(toml::Value::Integer(*i)),
        Value::Float(f) => Ok(toml::Value::Float(*f)),
        Value::String(s) => Ok(toml::Value::String(s.clone())),
        Value::Array(a) => {
            let arr = a
                .iter()
                .map(our_value_to_toml_value)
                .collect::<ConfigResult<Vec<_>>>()?;
            Ok(toml::Value::Array(arr))
        }
        Value::Object(o) => {
            let mut table = toml::map::Map::new();
            for (k, v) in o {
                table.insert(k.clone(), our_value_to_toml_value(v)?);
            }
            Ok(toml::Value::Table(table))
        }
    }
}
