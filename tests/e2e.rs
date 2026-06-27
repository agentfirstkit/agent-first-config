#![cfg(any(feature = "json", feature = "toml", feature = "yaml"))]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::bool_assert_comparison,
    clippy::approx_constant
)]
//! End-to-end integration tests for afconfig.

use agent_first_config::{add_keyed, get_path, remove_keyed, set_path, Format, KeyedList, Value};
use std::collections::BTreeMap;

#[test]
fn test_json_round_trip() {
    let json_str = r#"{"imap": {"host": "mail.example.com", "port": 993}}"#;

    let value = Format::Json.load(json_str).unwrap();

    // Get nested value
    let host = get_path(&value, "imap.host", &[]).unwrap();
    assert_eq!(host.as_str().unwrap(), "mail.example.com");

    // Set and save
    let mut value = value;
    set_path(&mut value, "imap.port", &["587".to_string()], &[]).unwrap();

    let output = Format::Json.save(&value).unwrap();
    let reloaded = Format::Json.load(&output).unwrap();
    let port = get_path(&reloaded, "imap.port", &[]).unwrap();
    assert_eq!(port.as_integer().unwrap(), 587);
}

#[test]
fn test_toml_round_trip() {
    let toml_str = r#"
[database]
host = "localhost"
port = 5432
"#;

    let value = Format::Toml.load(toml_str).unwrap();

    let host = get_path(&value, "database.host", &[]).unwrap();
    assert_eq!(host.as_str().unwrap(), "localhost");

    let mut value = value;
    set_path(&mut value, "database.port", &["3306".to_string()], &[]).unwrap();

    let output = Format::Toml.save(&value).unwrap();
    let reloaded = Format::Toml.load(&output).unwrap();
    let port = get_path(&reloaded, "database.port", &[]).unwrap();
    assert_eq!(port.as_integer().unwrap(), 3306);
}

#[test]
fn test_yaml_round_trip() {
    let yaml_str = r#"
server:
  host: localhost
  port: 8080
"#;

    let value = Format::Yaml.load(yaml_str).unwrap();

    let host = get_path(&value, "server.host", &[]).unwrap();
    assert_eq!(host.as_str().unwrap(), "localhost");

    let mut value = value;
    set_path(&mut value, "server.port", &["9000".to_string()], &[]).unwrap();

    let output = Format::Yaml.save(&value).unwrap();
    let reloaded = Format::Yaml.load(&output).unwrap();
    let port = get_path(&reloaded, "server.port", &[]).unwrap();
    assert_eq!(port.as_integer().unwrap(), 9000);
}

#[test]
fn test_keyed_list_add_and_remove() {
    let mut root = Value::Object(BTreeMap::new());

    let keyed_lists = [KeyedList {
        prefix: "identities",
        slug_field: "identity",
    }];

    // Initialize array
    if let Some(obj) = root.as_object_mut() {
        obj.insert("identities".to_string(), Value::Array(vec![]));
    }

    // Add first identity
    add_keyed(
        &mut root,
        "identities",
        "alice",
        &keyed_lists,
        None,
        &[
            ("email".to_string(), "alice@example.com".to_string()),
            ("name".to_string(), "Alice".to_string()),
        ],
    )
    .unwrap();

    // Verify it was added
    let alice_email = get_path(&root, "identities.alice.email", &keyed_lists).unwrap();
    assert_eq!(alice_email.as_str().unwrap(), "alice@example.com");

    // Add second identity
    add_keyed(
        &mut root,
        "identities",
        "bob",
        &keyed_lists,
        None,
        &[("email".to_string(), "bob@example.com".to_string())],
    )
    .unwrap();

    let arr = root.get("identities").unwrap().as_array().unwrap();
    assert_eq!(arr.len(), 2);

    // Remove first identity
    remove_keyed(&mut root, "identities", "alice", &keyed_lists).unwrap();

    let arr = root.get("identities").unwrap().as_array().unwrap();
    assert_eq!(arr.len(), 1);

    let bob_email = get_path(&root, "identities.bob.email", &keyed_lists).unwrap();
    assert_eq!(bob_email.as_str().unwrap(), "bob@example.com");
}

#[test]
fn test_greedy_key_matching() {
    let json_str = r#"{"actions":{"case.add":{"steps":[{"move":"archive"}]}}}"#;

    let value = Format::Json.load(json_str).unwrap();

    // This path should match the "case.add" key (with a dot in it)
    let steps = get_path(&value, "actions.case.add.steps", &[]).unwrap();
    assert!(steps.is_array());
}

#[test]
fn test_type_coercion() {
    let mut root = Value::Object(BTreeMap::new());

    // Test integer coercion
    set_path(&mut root, "port", &["993".to_string()], &[]).unwrap();
    let port = get_path(&root, "port", &[]).unwrap();
    assert_eq!(port.as_integer(), Some(993));

    // Test bool coercion
    set_path(&mut root, "enabled", &["true".to_string()], &[]).unwrap();
    let enabled = get_path(&root, "enabled", &[]).unwrap();
    assert_eq!(enabled.as_bool(), Some(true));

    // Test float coercion
    set_path(&mut root, "timeout", &["3.14".to_string()], &[]).unwrap();
    let timeout = get_path(&root, "timeout", &[]).unwrap();
    match timeout.as_float() {
        Some(f) => assert!((f - 3.14).abs() < 0.01),
        None => panic!("expected float"),
    }

    // Test string coercion
    set_path(&mut root, "name", &["Alice".to_string()], &[]).unwrap();
    let name = get_path(&root, "name", &[]).unwrap();
    assert_eq!(name.as_str(), Some("Alice"));
}

#[test]
fn test_scalar_array_replacement() {
    let mut root = Value::Object(BTreeMap::new());

    // Multi-value set should create array
    set_path(
        &mut root,
        "tags",
        &["dev".to_string(), "staging".to_string(), "prod".to_string()],
        &[],
    )
    .unwrap();

    let tags = get_path(&root, "tags", &[]).unwrap();
    let arr = tags.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_str(), Some("dev"));
    assert_eq!(arr[1].as_str(), Some("staging"));
    assert_eq!(arr[2].as_str(), Some("prod"));
}

#[test]
fn test_nested_object_creation() {
    let mut root = Value::Object(BTreeMap::new());

    // Setting a deeply nested key should create intermediates
    set_path(
        &mut root,
        "server.database.connection.host",
        &["localhost".to_string()],
        &[],
    )
    .unwrap();

    let host = get_path(&root, "server.database.connection.host", &[]).unwrap();
    assert_eq!(host.as_str().unwrap(), "localhost");
}

#[test]
fn test_json_array_coercion() {
    let mut root = Value::Object(BTreeMap::new());

    // Passing JSON array string should be parsed
    set_path(&mut root, "config", &[r#"["a","b","c"]"#.to_string()], &[]).unwrap();

    let config = get_path(&root, "config", &[]).unwrap();
    let arr = config.as_array().unwrap();
    assert_eq!(arr.len(), 3);
}

#[test]
fn test_type_prefix_coercion() {
    let mut root = Value::Object(BTreeMap::new());

    // s: prefix should force string
    set_path(&mut root, "field1", &["s:true".to_string()], &[]).unwrap();
    let val = get_path(&root, "field1", &[]).unwrap();
    assert_eq!(val.as_str(), Some("true"));

    // b: prefix should force bool
    set_path(&mut root, "field2", &["b:1".to_string()], &[]).unwrap();
    let val = get_path(&root, "field2", &[]).unwrap();
    assert_eq!(val.as_bool(), Some(true));
}

#[test]
fn test_error_on_nonexistent_slug() {
    let mut root = Value::Object(BTreeMap::new());

    let keyed_lists = [KeyedList {
        prefix: "identities",
        slug_field: "identity",
    }];

    // Initialize array
    if let Some(obj) = root.as_object_mut() {
        obj.insert("identities".to_string(), Value::Array(vec![]));
    }

    // Try to remove nonexistent slug
    let result = remove_keyed(&mut root, "identities", "nonexistent", &keyed_lists);
    assert!(result.is_err());
}

#[test]
fn test_format_detection() {
    assert_eq!(
        Format::detect(std::path::Path::new("config.json")),
        Some(Format::Json)
    );
    assert_eq!(
        Format::detect(std::path::Path::new("config.toml")),
        Some(Format::Toml)
    );
    assert_eq!(
        Format::detect(std::path::Path::new("config.yaml")),
        Some(Format::Yaml)
    );
    assert_eq!(
        Format::detect(std::path::Path::new("config.yml")),
        Some(Format::Yaml)
    );
    assert_eq!(Format::detect(std::path::Path::new("config.txt")), None);
}

#[test]
fn test_file_operations() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = temp_dir.path().join("config.json");

    // Write initial config
    let initial = r#"{"app":{"name":"test","version":"1.0"}}"#;
    fs::write(&config_path, initial).expect("failed to write initial config");

    // Read and modify
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let mut value = Format::Json.load(&content).expect("failed to load JSON");
    // Use s: prefix to force string type (avoid coercion to float)
    set_path(&mut value, "app.version", &["s:2.0".to_string()], &[]).expect("failed to set path");

    // Write back
    let output = Format::Json.save(&value).expect("failed to save JSON");
    fs::write(&config_path, output).expect("failed to write updated config");

    // Verify
    let updated = fs::read_to_string(&config_path).expect("failed to read updated config");
    let reloaded = Format::Json
        .load(&updated)
        .expect("failed to load updated JSON");
    let version = get_path(&reloaded, "app.version", &[]).expect("failed to get version");
    assert_eq!(version.as_str().expect("version should be string"), "2.0");
}
