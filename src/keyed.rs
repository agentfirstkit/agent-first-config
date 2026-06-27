//! KeyedList operations for slug-based array access.

use crate::{ConfigError, ConfigResult, Value};

/// Declares that an array at `prefix` is keyed by `slug_field`.
///
/// Example: `KeyedList { prefix: "identities", slug_field: "identity" }`
/// enables path `identities.me.email` to find the element where
/// `element["identity"] == "me"`, then read/write `element["email"]`.
#[derive(Debug, Clone, Copy)]
pub struct KeyedList {
    pub prefix: &'static str,
    pub slug_field: &'static str,
}

/// Add a new element to a keyed list.
///
/// The new element is built in three layers:
/// 1. `seed` fields (if provided) — default template values
/// 2. `{ slug_field: slug }` — always set, overrides any slug value in seed
/// 3. explicit `fields` — override both seed and slug (except the slug field)
pub fn add_keyed(
    root: &mut Value,
    prefix: &str,
    slug: &str,
    keyed_lists: &[KeyedList],
    seed: Option<&Value>,
    fields: &[(String, String)],
) -> ConfigResult<()> {
    // Check that prefix is registered
    keyed_lists
        .iter()
        .find(|kl| kl.prefix == prefix)
        .ok_or_else(|| ConfigError::UnregisteredArray {
            path: prefix.to_string(),
        })?;

    // '.' is the path separator — a slug containing it would be unreachable via get/set_path.
    if slug.contains('.') {
        return Err(ConfigError::ParseError {
            format: "slug".to_string(),
            detail: format!("slug `{slug}` must not contain '.' (path separator)"),
        });
    }

    add_keyed_recursive(root, prefix, slug, seed, fields, keyed_lists)
}

fn add_keyed_recursive(
    current: &mut Value,
    path_remaining: &str,
    slug: &str,
    seed: Option<&Value>,
    fields: &[(String, String)],
    keyed_lists: &[KeyedList],
) -> ConfigResult<()> {
    use crate::coerce::coerce_scalar;

    if let Some(dot_pos) = path_remaining.find('.') {
        let segment = path_remaining[..dot_pos].to_string();
        let rest = &path_remaining[dot_pos + 1..];

        if let Value::Object(obj) = current {
            obj.entry(segment.clone())
                .or_insert_with(|| Value::Object(Default::default()));
            if let Some(next) = obj.get_mut(&segment) {
                add_keyed_recursive(next, rest, slug, seed, fields, keyed_lists)
            } else {
                Err(ConfigError::NotTraversable {
                    path: path_remaining.to_string(),
                    got: "entry failed".to_string(),
                })
            }
        } else {
            Err(ConfigError::NotTraversable {
                path: path_remaining.to_string(),
                got: "not an object".to_string(),
            })
        }
    } else {
        // Last segment — should be array
        if let Value::Object(obj) = current {
            obj.entry(path_remaining.to_string())
                .or_insert_with(|| Value::Array(Vec::new()));

            if let Some(arr) = obj.get_mut(path_remaining).and_then(|v| v.as_array_mut()) {
                let reg = keyed_lists
                    .iter()
                    .find(|kl| kl.prefix == path_remaining)
                    .ok_or_else(|| ConfigError::UnregisteredArray {
                        path: path_remaining.to_string(),
                    })?;

                // Reject duplicate slugs — callers that want upsert must remove first.
                let already_exists = arr.iter().any(|e| {
                    e.as_object()
                        .and_then(|o| o.get(reg.slug_field))
                        .and_then(|v| v.as_str())
                        == Some(slug)
                });
                if already_exists {
                    return Err(ConfigError::SlugAlreadyExists {
                        prefix: path_remaining.to_string(),
                        slug: slug.to_string(),
                    });
                }

                let mut new_elem = Value::Object(Default::default());
                if let Some(elem_obj) = new_elem.as_object_mut() {
                    // Layer 1: seed defaults
                    if let Some(seed_obj) = seed.and_then(|s| s.as_object()) {
                        for (k, v) in seed_obj {
                            if k != reg.slug_field {
                                elem_obj.insert(k.clone(), v.clone());
                            }
                        }
                    }
                    // Layer 2: slug field (always authoritative)
                    elem_obj.insert(reg.slug_field.to_string(), Value::String(slug.to_string()));
                    // Layer 3: explicit fields override seed
                    for (k, v) in fields {
                        elem_obj.insert(k.clone(), coerce_scalar(v));
                    }
                }
                arr.push(new_elem);
                Ok(())
            } else {
                Err(ConfigError::NotTraversable {
                    path: path_remaining.to_string(),
                    got: "not an array".to_string(),
                })
            }
        } else {
            Err(ConfigError::NotTraversable {
                path: path_remaining.to_string(),
                got: "not an object".to_string(),
            })
        }
    }
}

/// Remove an element from a keyed list by slug.
pub fn remove_keyed(
    root: &mut Value,
    prefix: &str,
    slug: &str,
    keyed_lists: &[KeyedList],
) -> ConfigResult<()> {
    // Check that prefix is registered
    keyed_lists
        .iter()
        .find(|kl| kl.prefix == prefix)
        .ok_or_else(|| ConfigError::UnregisteredArray {
            path: prefix.to_string(),
        })?;

    remove_keyed_recursive(root, prefix, slug, keyed_lists)
}

fn remove_keyed_recursive(
    current: &mut Value,
    path_remaining: &str,
    slug: &str,
    keyed_lists: &[KeyedList],
) -> ConfigResult<()> {
    if let Some(dot_pos) = path_remaining.find('.') {
        let segment = path_remaining[..dot_pos].to_string();
        let rest = &path_remaining[dot_pos + 1..];

        if let Value::Object(obj) = current {
            if obj.contains_key(&segment) {
                if let Some(next) = obj.get_mut(&segment) {
                    remove_keyed_recursive(next, rest, slug, keyed_lists)
                } else {
                    Err(ConfigError::PathNotFound {
                        path: path_remaining.to_string(),
                    })
                }
            } else {
                Err(ConfigError::PathNotFound {
                    path: path_remaining.to_string(),
                })
            }
        } else {
            Err(ConfigError::NotTraversable {
                path: path_remaining.to_string(),
                got: "not an object".to_string(),
            })
        }
    } else {
        // Last segment — should be array
        if let Value::Object(obj) = current {
            if let Some(arr) = obj.get_mut(path_remaining).and_then(|v| v.as_array_mut()) {
                let registration = keyed_lists
                    .iter()
                    .find(|kl| kl.prefix == path_remaining)
                    .ok_or_else(|| ConfigError::UnregisteredArray {
                        path: path_remaining.to_string(),
                    })?;

                let original_len = arr.len();
                arr.retain(|elem| {
                    if let Some(elem_obj) = elem.as_object() {
                        if let Some(Value::String(elem_slug)) =
                            elem_obj.get(registration.slug_field)
                        {
                            elem_slug != slug
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                });

                if arr.len() == original_len {
                    return Err(ConfigError::SlugNotFound {
                        prefix: path_remaining.to_string(),
                        slug: slug.to_string(),
                    });
                }

                Ok(())
            } else {
                Err(ConfigError::PathNotFound {
                    path: path_remaining.to_string(),
                })
            }
        } else {
            Err(ConfigError::NotTraversable {
                path: path_remaining.to_string(),
                got: "not an object".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;

    #[test]
    fn test_add_keyed() {
        let mut root = Value::Object(Default::default());
        let keyed = [KeyedList {
            prefix: "identities",
            slug_field: "identity",
        }];

        root.as_object_mut()
            .unwrap()
            .insert("identities".to_string(), Value::Array(vec![]));

        add_keyed(
            &mut root,
            "identities",
            "me",
            &keyed,
            None,
            &[
                ("email".to_string(), "me@example.com".to_string()),
                ("name".to_string(), "Me".to_string()),
            ],
        )
        .unwrap();

        let arr = root.get("identities").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);

        let elem = &arr[0];
        assert_eq!(elem.get("identity").unwrap().as_str().unwrap(), "me");
        assert_eq!(
            elem.get("email").unwrap().as_str().unwrap(),
            "me@example.com"
        );
    }

    #[test]
    fn test_add_keyed_with_seed() {
        let mut root = Value::Object(Default::default());
        let keyed = [KeyedList {
            prefix: "identities",
            slug_field: "identity",
        }];
        root.as_object_mut()
            .unwrap()
            .insert("identities".to_string(), Value::Array(vec![]));

        let mut seed_obj = std::collections::BTreeMap::new();
        seed_obj.insert("enabled".to_string(), Value::Bool(true));
        seed_obj.insert("role".to_string(), Value::String("user".to_string()));
        seed_obj.insert(
            "email".to_string(),
            Value::String("default@example.com".to_string()),
        );
        let seed = Value::Object(seed_obj);

        add_keyed(
            &mut root,
            "identities",
            "alice",
            &keyed,
            Some(&seed),
            &[("email".to_string(), "alice@example.com".to_string())], // overrides seed
        )
        .unwrap();

        let elem = &root.get("identities").unwrap().as_array().unwrap()[0];
        assert_eq!(elem.get("identity").unwrap().as_str().unwrap(), "alice");
        assert_eq!(elem.get("role").unwrap().as_str().unwrap(), "user"); // from seed
        assert!(elem.get("enabled").unwrap().as_bool().unwrap()); // from seed
        assert_eq!(
            elem.get("email").unwrap().as_str().unwrap(),
            "alice@example.com"
        ); // fields override seed
    }

    #[test]
    fn test_remove_keyed() {
        let mut root = Value::Object(Default::default());
        let keyed = [KeyedList {
            prefix: "identities",
            slug_field: "identity",
        }];

        let mut elem1 = Value::Object(Default::default());
        elem1
            .as_object_mut()
            .unwrap()
            .insert("identity".to_string(), Value::String("me".to_string()));

        let mut elem2 = Value::Object(Default::default());
        elem2
            .as_object_mut()
            .unwrap()
            .insert("identity".to_string(), Value::String("other".to_string()));

        root.as_object_mut()
            .unwrap()
            .insert("identities".to_string(), Value::Array(vec![elem1, elem2]));

        remove_keyed(&mut root, "identities", "me", &keyed).unwrap();

        let arr = root.get("identities").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].get("identity").unwrap().as_str().unwrap(), "other");
    }
}
