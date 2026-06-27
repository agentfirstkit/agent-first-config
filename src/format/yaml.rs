//! YAML format backend (re-serialize only, comments not preserved).

use crate::{ConfigError, ConfigResult, Value};

pub fn load(content: &str) -> ConfigResult<Value> {
    serde_yaml::from_str::<serde_yaml::Value>(content)
        .map(value_to_our_value)
        .map_err(|e| ConfigError::ParseError {
            format: "YAML".to_string(),
            detail: e.to_string(),
        })
}

pub fn save(value: &Value) -> ConfigResult<String> {
    let yaml_val = our_value_to_yaml_value(value)?;
    serde_yaml::to_string(&yaml_val).map_err(|e| ConfigError::ParseError {
        format: "YAML".to_string(),
        detail: e.to_string(),
    })
}

fn value_to_our_value(v: serde_yaml::Value) -> Value {
    match v {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(u) = n.as_u64() {
                Value::Integer(u as i64)
            } else {
                Value::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_yaml::Value::String(s) => Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            Value::Array(seq.into_iter().map(value_to_our_value).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut obj = std::collections::BTreeMap::new();
            for (k, v) in map {
                if let serde_yaml::Value::String(key) = k {
                    obj.insert(key, value_to_our_value(v));
                }
            }
            Value::Object(obj)
        }
        serde_yaml::Value::Tagged(t) => {
            // Tagged values: recurse on inner value
            value_to_our_value(t.value)
        }
    }
}

fn our_value_to_yaml_value(v: &Value) -> ConfigResult<serde_yaml::Value> {
    match v {
        Value::Null => Ok(serde_yaml::Value::Null),
        Value::Bool(b) => Ok(serde_yaml::Value::Bool(*b)),
        Value::Integer(i) => Ok(serde_yaml::Value::Number((*i).into())),
        Value::Float(f) => {
            // serde_yaml doesn't have Number::from_f64, just convert to string
            Ok(serde_yaml::Value::String(f.to_string()))
        }
        Value::String(s) => Ok(serde_yaml::Value::String(s.clone())),
        Value::Array(a) => {
            let seq = a
                .iter()
                .map(our_value_to_yaml_value)
                .collect::<ConfigResult<Vec<_>>>()?;
            Ok(serde_yaml::Value::Sequence(seq))
        }
        Value::Object(o) => {
            let mut mapping = serde_yaml::Mapping::new();
            for (k, v) in o {
                mapping.insert(
                    serde_yaml::Value::String(k.clone()),
                    our_value_to_yaml_value(v)?,
                );
            }
            Ok(serde_yaml::Value::Mapping(mapping))
        }
    }
}
