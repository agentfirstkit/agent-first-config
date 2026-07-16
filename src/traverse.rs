//! Core dot-path traversal for get/set operations.

use crate::{ConfigError, ConfigResult, Value, keyed::KeyedList, path::parse_path};

/// Get a value at the given dot-path.
///
/// Handles:
/// - Object field access (any level of nesting)
/// - KeyedList access (Vec<T> slug routing)
/// - Greedy key matching for keys containing '.'
pub fn get_path_ref<'a>(
    root: &'a Value,
    path: &str,
    keyed_lists: &[KeyedList<'_>],
) -> ConfigResult<&'a Value> {
    if path.is_empty() {
        return Err(ConfigError::EmptyPath);
    }

    let segments = parse_path(path)?;
    let mut current = root;
    let mut accumulated_prefix = String::new();
    let mut seg_idx = 0;

    while seg_idx < segments.len() {
        let current_seg = segments[seg_idx].as_str();

        match current {
            Value::Object(obj) => {
                // Try exact match first
                if let Some(next) = obj.get(current_seg) {
                    if !accumulated_prefix.is_empty() {
                        accumulated_prefix.push('.');
                    }
                    accumulated_prefix.push_str(current_seg);
                    current = next;
                    seg_idx += 1;
                } else {
                    return Err(ConfigError::UnknownSegment {
                        path: path.to_string(),
                        segment: current_seg.to_string(),
                    });
                }
            }
            Value::Array(arr) => {
                // Numeric index takes priority over keyed-list slug.
                if let Ok(arr_idx) = current_seg.parse::<usize>() {
                    let elem = arr
                        .get(arr_idx)
                        .ok_or_else(|| ConfigError::IndexOutOfBounds {
                            path: accumulated_prefix.clone(),
                            index: arr_idx,
                            len: arr.len(),
                        })?;
                    if !accumulated_prefix.is_empty() {
                        accumulated_prefix.push('.');
                    }
                    accumulated_prefix.push_str(current_seg);
                    current = elem;
                    seg_idx += 1;
                } else {
                    let registration = keyed_lists
                        .iter()
                        .find(|kl| keyed_prefix_matches(kl, &accumulated_prefix))
                        .ok_or_else(|| ConfigError::UnregisteredArray {
                            path: accumulated_prefix.clone(),
                        })?;

                    let slug = current_seg;
                    let elem = arr
                        .iter()
                        .find(|e| {
                            if let Some(obj) = e.as_object() {
                                if let Some(Value::String(s)) = obj.get(registration.slug_field) {
                                    s == slug
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        })
                        .ok_or_else(|| ConfigError::SlugNotFound {
                            prefix: accumulated_prefix.clone(),
                            slug: slug.to_string(),
                        })?;

                    current = elem;
                    accumulated_prefix.push('.');
                    accumulated_prefix.push_str(slug);
                    seg_idx += 1;
                }
            }
            _ => {
                return Err(ConfigError::NotTraversable {
                    path: path.to_string(),
                    got: format!("{:?}", current),
                });
            }
        }
    }

    Ok(current)
}

/// Get a cloned value at the given dot-path.
pub fn get_path(root: &Value, path: &str, keyed_lists: &[KeyedList<'_>]) -> ConfigResult<Value> {
    Ok(get_path_ref(root, path, keyed_lists)?.clone())
}

/// Set a value at the given dot-path, coercing the CLI strings toward the type
/// already present at the target (type-directed, never shape-guessed). The raw
/// `values` are carried to the leaf so coercion can see the existing value.
pub fn set_path(
    root: &mut Value,
    path: &str,
    values: &[String],
    keyed_lists: &[KeyedList<'_>],
) -> ConfigResult<()> {
    if path.is_empty() {
        return Err(ConfigError::EmptyPath);
    }

    let segments = parse_path(path)?;
    set_path_recursive(root, &segments, 0, &mut String::new(), keyed_lists, values)
}

fn set_path_recursive(
    current: &mut Value,
    segments: &[String],
    idx: usize,
    accumulated_prefix: &mut String,
    keyed_lists: &[KeyedList<'_>],
    values: &[String],
) -> ConfigResult<()> {
    if idx >= segments.len() {
        return Err(ConfigError::EmptyPath);
    }

    let current_seg = segments[idx].as_str();
    let is_last = idx == segments.len() - 1;

    match current {
        Value::Object(obj) => {
            // Path parsing makes dotted keys explicit via `\\.`.
            let key_to_use = current_seg.to_string();

            let segments_to_consume = 1;

            if is_last {
                // At leaf: coerce toward the type already stored here, then set.
                let new_value = crate::coerce::coerce_values_typed(values, obj.get(&key_to_use))?;
                obj.insert(key_to_use, new_value);
                Ok(())
            } else {
                // Not at leaf: ensure key exists and recurse
                let next_idx = idx + segments_to_consume;
                if next_idx >= segments.len() {
                    return Err(ConfigError::EmptyPath);
                }

                if !accumulated_prefix.is_empty() {
                    accumulated_prefix.push('.');
                }
                accumulated_prefix.push_str(&key_to_use);

                // Use entry API to avoid double borrow
                use std::collections::btree_map::Entry;
                match obj.entry(key_to_use) {
                    Entry::Occupied(mut ent) => set_path_recursive(
                        ent.get_mut(),
                        segments,
                        next_idx,
                        accumulated_prefix,
                        keyed_lists,
                        values,
                    ),
                    Entry::Vacant(ent) => {
                        let mut new_obj = Value::Object(Default::default());
                        let result = set_path_recursive(
                            &mut new_obj,
                            segments,
                            next_idx,
                            accumulated_prefix,
                            keyed_lists,
                            values,
                        );
                        if result.is_ok() {
                            ent.insert(new_obj);
                        }
                        result
                    }
                }
            }
        }
        Value::Array(arr) => {
            // Numeric index takes priority over keyed-list slug.
            if let Ok(arr_idx) = current_seg.parse::<usize>() {
                if arr_idx >= arr.len() {
                    return Err(ConfigError::IndexOutOfBounds {
                        path: accumulated_prefix.clone(),
                        index: arr_idx,
                        len: arr.len(),
                    });
                }
                if is_last {
                    let existing = Some(&arr[arr_idx]);
                    let new_value = crate::coerce::coerce_values_typed(values, existing)?;
                    arr[arr_idx] = new_value;
                    Ok(())
                } else {
                    accumulated_prefix.push('.');
                    accumulated_prefix.push_str(current_seg);
                    set_path_recursive(
                        &mut arr[arr_idx],
                        segments,
                        idx + 1,
                        accumulated_prefix,
                        keyed_lists,
                        values,
                    )
                }
            } else {
                let registration = keyed_lists
                    .iter()
                    .find(|kl| keyed_prefix_matches(kl, accumulated_prefix))
                    .ok_or_else(|| ConfigError::UnregisteredArray {
                        path: accumulated_prefix.clone(),
                    })?;

                let slug = current_seg;
                let elem_idx = arr
                    .iter()
                    .position(|e| {
                        if let Some(elem_obj) = e.as_object() {
                            if let Some(Value::String(s)) = elem_obj.get(registration.slug_field) {
                                s == slug
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    })
                    .ok_or_else(|| ConfigError::SlugNotFound {
                        prefix: accumulated_prefix.clone(),
                        slug: slug.to_string(),
                    })?;

                if is_last {
                    Err(ConfigError::UnsupportedOperation {
                        format: "keyed list".to_string(),
                        operation: "set".to_string(),
                        detail:
                            "a keyed-list slug resolves to an element; set a child field instead"
                                .to_string(),
                    })
                } else {
                    accumulated_prefix.push('.');
                    accumulated_prefix.push_str(slug);
                    set_path_recursive(
                        &mut arr[elem_idx],
                        segments,
                        idx + 1,
                        accumulated_prefix,
                        keyed_lists,
                        values,
                    )
                }
            }
        }
        _ => Err(ConfigError::NotTraversable {
            path: accumulated_prefix.clone(),
            got: format!("{:?}", current),
        }),
    }
}

fn keyed_prefix_matches(registration: &KeyedList<'_>, semantic_prefix: &str) -> bool {
    registration.prefix == semantic_prefix
        || crate::parse_path(registration.prefix)
            .ok()
            .is_some_and(|segments| segments.join(".") == semantic_prefix)
}

/// Append scalar values to an existing (or absent) array at the given dot-path.
pub fn add_scalar(root: &mut Value, path: &str, values: &[String]) -> ConfigResult<()> {
    if path.is_empty() {
        return Err(ConfigError::EmptyPath);
    }
    let segments = parse_path(path)?;
    add_scalar_recursive(root, &segments, 0, &mut String::new(), values)
}

fn add_scalar_recursive(
    current: &mut Value,
    segments: &[String],
    idx: usize,
    accumulated_prefix: &mut String,
    values: &[String],
) -> ConfigResult<()> {
    if idx >= segments.len() {
        return Err(ConfigError::EmptyPath);
    }
    let current_seg = segments[idx].as_str();
    let is_last = idx == segments.len() - 1;

    match current {
        Value::Object(obj) => {
            let key_to_use = current_seg.to_string();
            let segments_to_consume = 1;

            if is_last {
                let arr = obj
                    .entry(key_to_use)
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(arr_inner) = arr {
                    for v in values {
                        arr_inner.push(crate::coerce::coerce_scalar(v));
                    }
                    Ok(())
                } else {
                    Err(ConfigError::NotTraversable {
                        path: current_seg.to_string(),
                        got: format!("{:?}", arr),
                    })
                }
            } else {
                let next_idx = idx + segments_to_consume;
                if !accumulated_prefix.is_empty() {
                    accumulated_prefix.push('.');
                }
                accumulated_prefix.push_str(&key_to_use);
                use std::collections::btree_map::Entry;
                match obj.entry(key_to_use) {
                    Entry::Occupied(mut ent) => add_scalar_recursive(
                        ent.get_mut(),
                        segments,
                        next_idx,
                        accumulated_prefix,
                        values,
                    ),
                    Entry::Vacant(ent) => {
                        let mut new_obj = Value::Object(Default::default());
                        let result = add_scalar_recursive(
                            &mut new_obj,
                            segments,
                            next_idx,
                            accumulated_prefix,
                            values,
                        );
                        if result.is_ok() {
                            ent.insert(new_obj);
                        }
                        result
                    }
                }
            }
        }
        _ => Err(ConfigError::NotTraversable {
            path: accumulated_prefix.clone(),
            got: format!("{:?}", current),
        }),
    }
}

/// Remove matching scalar values from an array at the given dot-path.
pub fn remove_scalar(root: &mut Value, path: &str, values: &[String]) -> ConfigResult<()> {
    if path.is_empty() {
        return Err(ConfigError::EmptyPath);
    }
    let segments = parse_path(path)?;
    remove_scalar_recursive(root, &segments, 0, &mut String::new(), values)
}

fn remove_scalar_recursive(
    current: &mut Value,
    segments: &[String],
    idx: usize,
    accumulated_prefix: &mut String,
    values: &[String],
) -> ConfigResult<()> {
    if idx >= segments.len() {
        return Err(ConfigError::EmptyPath);
    }
    let current_seg = segments[idx].as_str();
    let is_last = idx == segments.len() - 1;

    match current {
        Value::Object(obj) => {
            let key_to_use = current_seg.to_string();
            let segments_to_consume = 1;

            if is_last {
                if let Some(arr) = obj.get_mut(&key_to_use) {
                    if let Value::Array(arr_inner) = arr {
                        let coerced: Vec<_> = values
                            .iter()
                            .map(|v| crate::coerce::coerce_scalar(v))
                            .collect();
                        arr_inner.retain(|elem| !coerced.contains(elem));
                        Ok(())
                    } else {
                        Err(ConfigError::NotTraversable {
                            path: key_to_use,
                            got: format!("{:?}", arr),
                        })
                    }
                } else {
                    Err(ConfigError::PathNotFound { path: key_to_use })
                }
            } else {
                let next_idx = idx + segments_to_consume;
                if !accumulated_prefix.is_empty() {
                    accumulated_prefix.push('.');
                }
                accumulated_prefix.push_str(&key_to_use);
                if let Some(next) = obj.get_mut(&key_to_use) {
                    remove_scalar_recursive(next, segments, next_idx, accumulated_prefix, values)
                } else {
                    Err(ConfigError::PathNotFound {
                        path: accumulated_prefix.clone(),
                    })
                }
            }
        }
        _ => Err(ConfigError::NotTraversable {
            path: accumulated_prefix.clone(),
            got: format!("{:?}", current),
        }),
    }
}

/// Remove the key at the given dot-path from its parent object.
pub fn remove_path(root: &mut Value, path: &str) -> ConfigResult<()> {
    if path.is_empty() {
        return Err(ConfigError::EmptyPath);
    }
    let segments = parse_path(path)?;
    remove_path_recursive(root, &segments, 0, &mut String::new())
}

fn remove_path_recursive(
    current: &mut Value,
    segments: &[String],
    idx: usize,
    accumulated_prefix: &mut String,
) -> ConfigResult<()> {
    if idx >= segments.len() {
        return Err(ConfigError::EmptyPath);
    }
    let current_seg = segments[idx].as_str();
    let is_last = idx == segments.len() - 1;

    match current {
        Value::Object(obj) => {
            let key_to_use = current_seg.to_string();
            let segments_to_consume = 1;

            if is_last {
                if obj.remove(&key_to_use).is_none() {
                    return Err(ConfigError::PathNotFound { path: key_to_use });
                }
                Ok(())
            } else {
                let next_idx = idx + segments_to_consume;
                if !accumulated_prefix.is_empty() {
                    accumulated_prefix.push('.');
                }
                accumulated_prefix.push_str(&key_to_use);
                if let Some(next) = obj.get_mut(&key_to_use) {
                    remove_path_recursive(next, segments, next_idx, accumulated_prefix)
                } else {
                    Err(ConfigError::PathNotFound {
                        path: accumulated_prefix.clone(),
                    })
                }
            }
        }
        Value::Array(arr) => {
            if let Ok(arr_idx) = current_seg.parse::<usize>() {
                if arr_idx >= arr.len() {
                    return Err(ConfigError::IndexOutOfBounds {
                        path: accumulated_prefix.clone(),
                        index: arr_idx,
                        len: arr.len(),
                    });
                }
                if is_last {
                    arr.remove(arr_idx);
                    Ok(())
                } else {
                    accumulated_prefix.push('.');
                    accumulated_prefix.push_str(current_seg);
                    remove_path_recursive(&mut arr[arr_idx], segments, idx + 1, accumulated_prefix)
                }
            } else {
                Err(ConfigError::UnregisteredArray {
                    path: accumulated_prefix.clone(),
                })
            }
        }
        _ => Err(ConfigError::NotTraversable {
            path: accumulated_prefix.clone(),
            got: format!("{:?}", current),
        }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        clippy::expect_used,
        clippy::bool_assert_comparison
    )]
    use super::*;

    fn make_test_object() -> Value {
        let mut root = Value::Object(Default::default());
        let mut imap = Value::Object(Default::default());
        imap.as_object_mut().unwrap().insert(
            "host".to_string(),
            Value::String("mail.example.com".to_string()),
        );
        imap.as_object_mut()
            .unwrap()
            .insert("port".to_string(), Value::Integer(993));

        root.as_object_mut()
            .unwrap()
            .insert("imap".to_string(), imap);

        root
    }

    #[test]
    fn test_get_path_simple() {
        let root = make_test_object();
        let result = get_path(&root, "imap.host", &[]).unwrap();
        assert_eq!(result.as_str().unwrap(), "mail.example.com");
    }

    #[test]
    fn test_get_path_integer() {
        let root = make_test_object();
        let result = get_path(&root, "imap.port", &[]).unwrap();
        assert_eq!(result.as_integer().unwrap(), 993);
    }

    #[test]
    fn test_set_path_new_key() {
        let mut root = make_test_object();
        set_path(&mut root, "imap.tls", &["true".to_string()], &[]).unwrap();

        let result = get_path(&root, "imap.tls", &[]).unwrap();
        assert_eq!(result.as_bool().unwrap(), true);
    }

    #[test]
    fn test_set_path_overwrite() {
        let mut root = make_test_object();
        set_path(&mut root, "imap.port", &["587".to_string()], &[]).unwrap();

        let result = get_path(&root, "imap.port", &[]).unwrap();
        assert_eq!(result.as_integer().unwrap(), 587);
    }

    #[test]
    fn test_set_path_array_value() {
        let mut root = Value::Object(Default::default());
        set_path(
            &mut root,
            "tags",
            &["dev".to_string(), "staging".to_string()],
            &[],
        )
        .unwrap();

        let result = get_path(&root, "tags", &[]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    fn make_steps_object() -> Value {
        // { "steps": [{"name": "a", "port": 1}, {"name": "b", "port": 2}] }
        let mut root = Value::Object(Default::default());
        let mut s0 = Value::Object(Default::default());
        s0.as_object_mut()
            .unwrap()
            .insert("name".to_string(), Value::String("a".to_string()));
        s0.as_object_mut()
            .unwrap()
            .insert("port".to_string(), Value::Integer(1));
        let mut s1 = Value::Object(Default::default());
        s1.as_object_mut()
            .unwrap()
            .insert("name".to_string(), Value::String("b".to_string()));
        s1.as_object_mut()
            .unwrap()
            .insert("port".to_string(), Value::Integer(2));
        root.as_object_mut()
            .unwrap()
            .insert("steps".to_string(), Value::Array(vec![s0, s1]));
        root
    }

    #[test]
    fn test_get_path_numeric_index() {
        let root = make_steps_object();
        let name = get_path(&root, "steps.0.name", &[]).unwrap();
        assert_eq!(name.as_str().unwrap(), "a");
        let port = get_path(&root, "steps.1.port", &[]).unwrap();
        assert_eq!(port.as_integer().unwrap(), 2);
    }

    #[test]
    fn test_set_path_numeric_index() {
        let mut root = make_steps_object();
        set_path(&mut root, "steps.0.port", &["99".to_string()], &[]).unwrap();
        let result = get_path(&root, "steps.0.port", &[]).unwrap();
        assert_eq!(result.as_integer().unwrap(), 99);
        // other element unchanged
        let other = get_path(&root, "steps.1.port", &[]).unwrap();
        assert_eq!(other.as_integer().unwrap(), 2);
    }

    #[test]
    fn test_get_path_index_out_of_bounds() {
        let root = make_steps_object();
        let err = get_path(&root, "steps.5.name", &[]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::IndexOutOfBounds {
                index: 5,
                len: 2,
                ..
            }
        ));
    }

    #[test]
    fn test_remove_path_numeric_index() {
        let mut root = make_steps_object();
        remove_path(&mut root, "steps.0").unwrap();
        let arr = get_path(&root, "steps", &[]).unwrap();
        let arr = arr.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].get("name").unwrap().as_str().unwrap(), "b");
    }
}
