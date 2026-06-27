//! Coerce CLI string arguments to Value.

use crate::{ConfigError, ConfigResult, Value};

/// Honor an explicit type prefix, if present:
/// - `s:value` → String (no coercion)
/// - `b:value` → Bool (true/yes/on/1)
/// - `i:value` → Integer
/// - `j:value` → JSON parse
///
/// Returns `None` when `s` carries no recognized prefix, so the caller can apply
/// its own (typed or shape-based) coercion.
fn explicit_prefix(s: &str) -> Option<Value> {
    if let Some(rest) = s.strip_prefix("s:") {
        return Some(Value::String(rest.to_string()));
    }
    if let Some(rest) = s.strip_prefix("b:") {
        return Some(Value::Bool(matches!(
            rest.to_lowercase().as_str(),
            "true" | "yes" | "on" | "1"
        )));
    }
    if let Some(rest) = s.strip_prefix("i:") {
        return Some(
            rest.parse::<i64>()
                .map(Value::Integer)
                .unwrap_or_else(|_| Value::String(s.to_string())),
        );
    }
    if let Some(_rest) = s.strip_prefix("j:") {
        #[cfg(feature = "json")]
        {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(_rest) {
                return Some(Value::from(v));
            }
        }
        return Some(Value::String(s.to_string()));
    }
    None
}

/// A bare scalar with no type context: parse an explicit `[...]`/`{...}` JSON
/// literal, otherwise keep it as a String. Deliberately does NOT guess numbers
/// or bools from shape — that is the job of type-directed coercion.
fn json_or_string(s: &str) -> Value {
    if (s.starts_with('[') || s.starts_with('{')) && s.trim_end().ends_with([']', '}']) {
        #[cfg(feature = "json")]
        {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
                return Value::from(v);
            }
        }
    }
    Value::String(s.to_string())
}

/// Coerce a single CLI string toward the type already present at the target
/// (`existing`) — never by guessing the string's shape. An explicit prefix
/// always wins, and `"null"` always clears. When the target is a typed scalar
/// the string is parsed into that type (falling back to String so the consumer's
/// deserialize step produces a precise error); when the target is absent/null or
/// a container, only an explicit JSON literal is structured, otherwise it stays a
/// String.
pub fn coerce_scalar_typed(s: &str, existing: Option<&Value>) -> Value {
    if let Some(v) = explicit_prefix(s) {
        return v;
    }
    if s.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    match existing {
        Some(Value::Bool(_)) => s
            .parse::<bool>()
            .map(Value::Bool)
            .unwrap_or_else(|_| Value::String(s.to_string())),
        Some(Value::Integer(_)) => s
            .parse::<i64>()
            .map(Value::Integer)
            .unwrap_or_else(|_| Value::String(s.to_string())),
        Some(Value::Float(_)) => s
            .parse::<f64>()
            .map(Value::Float)
            .unwrap_or_else(|_| Value::String(s.to_string())),
        Some(Value::String(_)) => Value::String(s.to_string()),
        Some(_) => json_or_string(s),
        None => coerce_scalar(s),
    }
}

/// Type-directed coercion of a CLI value slice toward the type at `existing`.
///
/// - Empty slice → Error
/// - Single value → [`coerce_scalar_typed`]
/// - Multiple values → Array, each element coerced toward the existing array's
///   element type (String when unknown)
pub fn coerce_values_typed(values: &[String], existing: Option<&Value>) -> ConfigResult<Value> {
    if values.is_empty() {
        return Err(ConfigError::EmptyValues);
    }
    if values.len() == 1 {
        return Ok(coerce_scalar_typed(&values[0], existing));
    }
    let elem = existing.and_then(Value::as_array).and_then(|a| a.first());
    Ok(Value::Array(
        values
            .iter()
            .map(|v| coerce_scalar_typed(v, elem))
            .collect(),
    ))
}

/// Shape-based coercion with no type context: an explicit prefix wins, otherwise
/// `null`/`true`/`false`/integer/float/JSON are recognized by shape, falling back
/// to String. Used where no target type is available (e.g. keyed-list fields
/// without a default template).
pub fn coerce_scalar(s: &str) -> Value {
    if let Some(v) = explicit_prefix(s) {
        return v;
    }
    match s.to_lowercase().as_str() {
        "null" => return Value::Null,
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::Integer(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }
    json_or_string(s)
}

/// Coerce a slice of CLI values to Value via shape-based [`coerce_scalar`].
///
/// - Empty slice → Error
/// - Single value → coerce_scalar
/// - Multiple values → Array of coerced values
pub fn coerce_values(values: &[String]) -> ConfigResult<Value> {
    if values.is_empty() {
        return Err(ConfigError::EmptyValues);
    }

    if values.len() == 1 {
        return Ok(coerce_scalar(&values[0]));
    }

    Ok(Value::Array(
        values.iter().map(|v| coerce_scalar(v)).collect(),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;

    #[test]
    fn test_coerce_null() {
        assert_eq!(coerce_scalar("null"), Value::Null);
        assert_eq!(coerce_scalar("NULL"), Value::Null);
    }

    #[test]
    fn test_coerce_bool() {
        assert_eq!(coerce_scalar("true"), Value::Bool(true));
        assert_eq!(coerce_scalar("false"), Value::Bool(false));
        assert_eq!(coerce_scalar("TRUE"), Value::Bool(true));
    }

    #[test]
    fn test_coerce_integer() {
        assert_eq!(coerce_scalar("42"), Value::Integer(42));
        assert_eq!(coerce_scalar("-100"), Value::Integer(-100));
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_coerce_float() {
        match coerce_scalar("3.14") {
            Value::Float(f) => assert!((f - 3.14).abs() < 0.001),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn test_coerce_string() {
        assert_eq!(coerce_scalar("hello"), Value::String("hello".to_string()));
    }

    #[test]
    fn test_coerce_explicit_prefix_s() {
        assert_eq!(coerce_scalar("s:true"), Value::String("true".to_string()));
        assert_eq!(coerce_scalar("s:42"), Value::String("42".to_string()));
    }

    #[test]
    fn test_coerce_explicit_prefix_b() {
        assert_eq!(coerce_scalar("b:yes"), Value::Bool(true));
        assert_eq!(coerce_scalar("b:no"), Value::Bool(false));
    }

    #[test]
    fn test_coerce_explicit_prefix_i() {
        assert_eq!(coerce_scalar("i:999"), Value::Integer(999));
    }

    #[test]
    fn test_coerce_values_single() {
        let result = coerce_values(&["42".to_string()]).unwrap();
        assert_eq!(result, Value::Integer(42));
    }

    #[test]
    fn test_coerce_values_multiple() {
        let result =
            coerce_values(&["dev".to_string(), "staging".to_string(), "prod".to_string()]).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0], Value::String("dev".to_string()));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_coerce_values_empty_error() {
        assert!(coerce_values(&[]).is_err());
    }
}
