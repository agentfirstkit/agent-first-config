#![cfg(any(
    feature = "json",
    feature = "toml",
    feature = "yaml",
    feature = "dotenv",
    feature = "ini"
))]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::bool_assert_comparison,
    clippy::approx_constant
)]
//! End-to-end integration tests for afconfig.

use agent_first_config::{Format, KeyedList, Value, add_keyed, get_path, remove_keyed, set_path};
use std::collections::BTreeMap;

#[cfg(feature = "json")]
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

#[cfg(feature = "json")]
#[test]
fn test_json_scalar_edit_preserves_unrelated_source() {
    let source = "{\n  \"z\": 1e+3,\n  \"nested\": { \"keep\": \"\\u0061\", \"target\": 2 }\n}\n";
    let edited = agent_first_config::format::json::set_scalar_preserving(
        source,
        "nested.target",
        &Value::Integer(7),
    )
    .unwrap();
    assert_eq!(
        edited,
        "{\n  \"z\": 1e+3,\n  \"nested\": { \"keep\": \"\\u0061\", \"target\": 7 }\n}\n"
    );
}

#[cfg(feature = "json")]
#[test]
fn test_json_unset_preserves_unrelated_source() {
    let source = "{\n  \"keep\": 1e+3,\n  \"remove\": 2,\n  \"last\": \"x\"\n}\n";
    let edited = agent_first_config::format::json::unset_preserving(source, "remove").unwrap();
    assert_eq!(edited, "{\n  \"keep\": 1e+3,\n  \"last\": \"x\"\n}\n");
}

#[cfg(feature = "json")]
#[test]
fn test_json_golden_variants_preserve_untouched_source() {
    let compact = r#"{"keep":"\u0061","target":1e+3,"tail":[1,2]}"#;
    let edited = agent_first_config::format::json::set_scalar_preserving(
        compact,
        "target",
        &Value::Integer(7),
    )
    .unwrap();
    assert!(edited.contains("\\u0061"));
    assert!(edited.contains("[1,2]"));
    assert_eq!(edited, r#"{"keep":"\u0061","target":7,"tail":[1,2]}"#);

    let crlf = "{\r\n  \"keep\": 1e+3,\r\n  \"target\": 2\r\n}\r\n";
    let edited = agent_first_config::format::json::unset_preserving(crlf, "target").unwrap();
    assert_eq!(edited, "{\r\n  \"keep\": 1e+3\r\n}\r\n");
}

#[cfg(feature = "toml")]
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

#[cfg(feature = "toml")]
#[test]
fn test_toml_scalar_edit_preserves_comments_and_datetime() {
    let source = "# keep\n[database]\nport = 5432 # note\nwhen = 2024-01-01T00:00:00Z\n";
    let edited = agent_first_config::format::toml::set_scalar_preserving(
        source,
        "database.port",
        &Value::Integer(3306),
    )
    .unwrap();
    assert_eq!(
        edited,
        "# keep\n[database]\nport = 3306 # note\nwhen = 2024-01-01T00:00:00Z\n"
    );
}

#[cfg(feature = "toml")]
#[test]
fn test_toml_unset_preserves_comments() {
    let source = "# keep\n[database]\nremove = 1 # remove\nkeep = 2024-01-01T00:00:00Z\n";
    let edited =
        agent_first_config::format::toml::unset_preserving(source, "database.remove").unwrap();
    assert_eq!(edited, "# keep\n[database]\nkeep = 2024-01-01T00:00:00Z\n");
}

#[cfg(feature = "toml")]
#[test]
fn test_toml_golden_array_and_datetime_bytes() {
    let source = "# keep\nwhen = 2024-01-01T00:00:00Z\nvalues = [1, 2, 3]\ntarget = 1\n";
    let edited = agent_first_config::format::toml::set_scalar_preserving(
        source,
        "target",
        &Value::Integer(2),
    )
    .unwrap();
    assert!(edited.contains("when = 2024-01-01T00:00:00Z"));
    assert!(edited.contains("values = [1, 2, 3]"));
    assert!(edited.ends_with("target = 2\n"));

    let arrays =
        "global_target = 1\n\n[[servers]]\nname = \"one\"\n\n[[servers]]\nname = \"two\"\n";
    let edited = agent_first_config::format::toml::set_scalar_preserving(
        arrays,
        "global_target",
        &Value::Integer(2),
    )
    .unwrap();
    assert!(edited.contains("[[servers]]\nname = \"one\""));
    assert!(edited.contains("global_target = 2"), "{edited}");
    assert!(edited.contains("[[servers]]\nname = \"two\""), "{edited}");
}

#[cfg(all(feature = "toml", feature = "json", feature = "cli"))]
#[test]
fn test_atomic_failure_keeps_original_file() {
    use std::process::Command;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.toml");
    let original = "value = 1\nkeep = 2\n";
    std::fs::write(&path, original).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&path)
        .args(["set", "value", "j:null"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

#[cfg(feature = "toml")]
#[test]
fn test_toml_rejects_unrepresentable_null_and_u64() {
    let null_error = agent_first_config::format::toml::set_scalar_preserving(
        "value = 1\n",
        "value",
        &Value::Null,
    )
    .unwrap_err();
    assert!(null_error.to_string().contains("no null"));
    let unsigned_error = agent_first_config::format::toml::set_scalar_preserving(
        "value = 1\n",
        "value",
        &Value::Unsigned(u64::MAX),
    )
    .unwrap_err();
    assert!(unsigned_error.to_string().contains("exceeds TOML"));
}

#[cfg(feature = "yaml")]
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

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_scalar_edit_preserves_comments_and_float() {
    let source = "# keep\nserver:\n  host: localhost # host\n  ratio: 1.0\n";
    let edited = agent_first_config::format::yaml::set_scalar_preserving(
        source,
        "server.host",
        &Value::String("example.com".to_string()),
    )
    .unwrap();
    assert_eq!(
        edited,
        "# keep\nserver:\n  host: example.com # host\n  ratio: 1.0\n"
    );
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_unset_preserves_comments() {
    let source = "# keep\nserver:\n  remove: 1 # remove\n  keep: 1.0\n";
    let edited =
        agent_first_config::format::yaml::unset_preserving(source, "server.remove").unwrap();
    assert_eq!(edited, "# keep\nserver:\n  keep: 1.0\n");
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_golden_styles_and_crlf() {
    let source =
        "# keep\r\nroot:\r\n  quoted: 'old'\r\n  literal: |\r\n    unchanged\r\n  target: 1.0\r\n";
    let edited = agent_first_config::format::yaml::set_scalar_preserving(
        source,
        "root.target",
        &Value::Float(2.0),
    )
    .unwrap();
    assert!(edited.contains("# keep\r\n"));
    assert!(edited.contains("quoted: 'old'\r\n"));
    assert!(edited.contains("literal: |\r\n    unchanged\r\n"));
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_golden_flow_tag_anchor_and_alias_untouched() {
    let source = "defaults: &defaults {name: old}\ncopy: *defaults\ntarget: 1\nflow: [1, 2]\n";
    let edited = agent_first_config::format::yaml::set_scalar_preserving(
        source,
        "target",
        &Value::Integer(2),
    )
    .unwrap();
    assert!(edited.contains("defaults: &defaults {name: old}"));
    assert!(edited.contains("copy: *defaults"));
    assert!(edited.contains("flow: [1, 2]"));
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_keyed_collection_edit_preserves_unrelated_source() {
    let source = "# keep\nitems:\n  - id: a\n    name: A\nkeep: 1.0\n";
    let item = Value::Object(BTreeMap::from([
        ("id".to_string(), Value::String("b".to_string())),
        ("name".to_string(), Value::String("B".to_string())),
    ]));
    let added =
        agent_first_config::format::yaml::append_array_item_preserving(source, "items", &item)
            .unwrap();
    assert!(added.contains("keep: 1.0"));
    assert_eq!(
        Format::Yaml
            .load(&added)
            .unwrap()
            .get("items")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let removed =
        agent_first_config::format::yaml::remove_array_item_preserving(&added, "items", 1).unwrap();
    assert!(removed.contains("keep: 1.0"));
    assert_eq!(
        Format::Yaml
            .load(&removed)
            .unwrap()
            .get("items")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_cst_numeric_path_adapter_and_unsupported_escaped_keys() {
    let source = "items:\n  - name: first\n  - name: second\nkeep: 1.0\n";
    let edited = agent_first_config::format::yaml::set_scalar_preserving(
        source,
        "items.1.name",
        &Value::String("changed".to_string()),
    )
    .unwrap();
    assert!(edited.contains("name: changed"));
    assert!(edited.contains("keep: 1.0"));
    let error = agent_first_config::format::yaml::set_scalar_preserving(
        source,
        r"items.key\.with.dot",
        &Value::String("x".to_string()),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        agent_first_config::ConfigError::UnsupportedOperation { .. }
    ));
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_uses_strict_yaml_1_2_parsing() {
    let value = Format::Yaml.load("country: NO\nenabled: true\n").unwrap();
    assert_eq!(
        get_path(&value, "country", &[]).unwrap().as_str(),
        Some("NO")
    );
    assert_eq!(
        get_path(&value, "enabled", &[]).unwrap().as_bool(),
        Some(true)
    );

    assert!(Format::Yaml.load("name: first\nname: second\n").is_err());
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

#[cfg(feature = "json")]
#[test]
fn test_escaped_dotted_key_matching() {
    let json_str = r#"{"actions":{"case.add":{"steps":[{"move":"archive"}]}}}"#;

    let value = Format::Json.load(json_str).unwrap();

    // Dotted keys are addressed explicitly; path resolution is not greedy.
    let steps = get_path(&value, r"actions.case\.add.steps", &[]).unwrap();
    assert!(steps.is_array());
}

#[test]
fn test_escaped_keyed_list_prefix_routes_consistently() {
    let mut root = Value::Object(BTreeMap::from([(
        "group.list".to_string(),
        Value::Object(BTreeMap::from([(
            "items".to_string(),
            Value::Array(vec![]),
        )])),
    )]));
    let keyed_lists = [KeyedList {
        prefix: r"group\.list.items",
        slug_field: "id",
    }];
    add_keyed(
        &mut root,
        r"group\.list.items",
        "one",
        &keyed_lists,
        None,
        &[("name".to_string(), "first".to_string())],
    )
    .unwrap();
    assert_eq!(
        get_path(&root, r"group\.list.items.one.name", &keyed_lists)
            .unwrap()
            .as_str(),
        Some("first")
    );
    set_path(
        &mut root,
        r"group\.list.items.one.name",
        &["second".to_string()],
        &keyed_lists,
    )
    .unwrap();
    assert_eq!(
        get_path(&root, r"group\.list.items.one.name", &keyed_lists)
            .unwrap()
            .as_str(),
        Some("second")
    );
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

#[cfg(feature = "json")]
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
fn test_numeric_boundaries_do_not_narrow_unsigned_or_float() {
    let max = agent_first_config::coerce_scalar("18446744073709551615");
    assert_eq!(max.as_unsigned(), Some(u64::MAX));
    let precise = agent_first_config::coerce_scalar("9007199254740993");
    assert_eq!(precise.as_integer(), Some(9_007_199_254_740_993));
    let float = agent_first_config::coerce_scalar("3.0");
    assert!(matches!(float, Value::Float(value) if value == 3.0));
    assert_eq!(
        agent_first_config::coerce_scalar(&i64::MIN.to_string()).as_integer(),
        Some(i64::MIN)
    );
    assert_eq!(
        agent_first_config::coerce_scalar(&i64::MAX.to_string()).as_integer(),
        Some(i64::MAX)
    );
    assert_eq!(
        agent_first_config::coerce_scalar("9007199254740991").as_integer(),
        Some(9_007_199_254_740_991)
    );
    assert_eq!(
        agent_first_config::coerce_scalar("9007199254740993").as_integer(),
        Some(9_007_199_254_740_993)
    );
}

#[cfg(feature = "json")]
#[test]
fn test_json_unsigned_boundary_round_trip() {
    let value = Format::Json
        .load("{\"n\":18446744073709551615,\"f\":3.0}")
        .unwrap();
    assert_eq!(
        get_path(&value, "n", &[]).unwrap().as_unsigned(),
        Some(u64::MAX)
    );
    assert!(matches!(get_path(&value, "f", &[]).unwrap(), Value::Float(value) if value == 3.0));
    assert!(Format::Json.load("1e400").is_err());
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_unsigned_boundary_round_trip() {
    let value = Format::Yaml
        .load("n: 18446744073709551615\nf: 3.0\n")
        .unwrap();
    assert_eq!(
        get_path(&value, "n", &[]).unwrap().as_unsigned(),
        Some(u64::MAX)
    );
    assert!(matches!(get_path(&value, "f", &[]).unwrap(), Value::Float(value) if value == 3.0));
}

#[cfg(feature = "json")]
#[test]
fn test_json_numeric_boundary_matrix() {
    let source = format!(
        "{{\"min\":{},\"max\":{},\"above\":{},\"u\":{},\"p\":9007199254740993,\"f\":1e-10}}",
        i64::MIN,
        i64::MAX,
        i64::MAX as u128 + 1,
        u64::MAX
    );
    let value = Format::Json.load(&source).unwrap();
    assert_eq!(
        get_path(&value, "min", &[]).unwrap().as_integer(),
        Some(i64::MIN)
    );
    assert_eq!(
        get_path(&value, "max", &[]).unwrap().as_integer(),
        Some(i64::MAX)
    );
    assert_eq!(
        get_path(&value, "above", &[]).unwrap().as_unsigned(),
        Some(i64::MAX as u64 + 1)
    );
    assert_eq!(
        get_path(&value, "u", &[]).unwrap().as_unsigned(),
        Some(u64::MAX)
    );
    assert!(matches!(get_path(&value, "f", &[]).unwrap(), Value::Float(f) if f > 0.0));
    assert!(Format::Json.load("{\"bad\":1e400}").is_err());
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_numeric_boundary_matrix() {
    let source = format!(
        "min: {}\nmax: {}\nabove: {}\nu: {}\nprecise: 9007199254740993\n",
        i64::MIN,
        i64::MAX,
        i64::MAX as u128 + 1,
        u64::MAX
    );
    let value = Format::Yaml.load(&source).unwrap();
    assert_eq!(
        get_path(&value, "min", &[]).unwrap().as_integer(),
        Some(i64::MIN)
    );
    assert_eq!(
        get_path(&value, "max", &[]).unwrap().as_integer(),
        Some(i64::MAX)
    );
    assert_eq!(
        get_path(&value, "above", &[]).unwrap().as_unsigned(),
        Some(i64::MAX as u64 + 1)
    );
    assert_eq!(
        get_path(&value, "u", &[]).unwrap().as_unsigned(),
        Some(u64::MAX)
    );
}

#[cfg(feature = "toml")]
#[test]
fn test_toml_numeric_boundary_matrix() {
    let value = Format::Toml
        .load(&format!(
            "min = {}\nmax = {}\nprecise = 9007199254740993\n",
            i64::MIN,
            i64::MAX
        ))
        .unwrap();
    assert_eq!(
        get_path(&value, "min", &[]).unwrap().as_integer(),
        Some(i64::MIN)
    );
    assert_eq!(
        get_path(&value, "max", &[]).unwrap().as_integer(),
        Some(i64::MAX)
    );
    assert_eq!(
        get_path(&value, "precise", &[]).unwrap().as_integer(),
        Some(9_007_199_254_740_993)
    );
    assert!(Format::Toml.load("bad = 1e9999\n").is_err());
    assert!(
        agent_first_config::format::toml::set_scalar_preserving(
            "value = 1\n",
            "value",
            &Value::Float(f64::NAN)
        )
        .is_err()
    );
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

#[cfg(all(feature = "ini", not(feature = "json")))]
#[test]
fn test_disabled_backend_reports_required_feature() {
    let error = Format::Json.load("{}").unwrap_err();
    assert!(matches!(
        error,
        agent_first_config::ConfigError::UnsupportedOperation { .. }
    ));
    assert!(error.to_string().contains("feature `json`"));
}

#[cfg(feature = "json")]
#[test]
fn test_format_detection_json() {
    assert_eq!(
        Format::detect(std::path::Path::new("config.json")),
        Some(Format::Json)
    );
    assert_eq!(Format::detect(std::path::Path::new("config.txt")), None);
}

#[cfg(feature = "toml")]
#[test]
fn test_format_detection_toml() {
    assert_eq!(
        Format::detect(std::path::Path::new("config.toml")),
        Some(Format::Toml)
    );
}

#[cfg(feature = "yaml")]
#[test]
fn test_format_detection_yaml() {
    assert_eq!(
        Format::detect(std::path::Path::new("config.yaml")),
        Some(Format::Yaml)
    );
    assert_eq!(
        Format::detect(std::path::Path::new("config.yml")),
        Some(Format::Yaml)
    );
}

#[cfg(feature = "dotenv")]
#[test]
fn test_format_detection_dotenv() {
    for path in [
        ".env",
        ".env.local",
        ".env.test",
        ".env.example",
        "config.env",
        "CONFIG.ENV",
    ] {
        assert_eq!(
            Format::detect(std::path::Path::new(path)),
            Some(Format::Dotenv),
            "failed to detect {path}"
        );
    }
    assert_eq!(Format::detect(std::path::Path::new("config.txt")), None);
}

#[cfg(feature = "ini")]
#[test]
fn test_ini_core_v1_strings_and_duplicates() {
    let value = Format::Ini
        .load("[database]\r\nhost = localhost\r\nport=5432\r\n")
        .unwrap();
    assert_eq!(
        get_path(&value, "database.host", &[]).unwrap().as_str(),
        Some("localhost")
    );
    assert_eq!(
        get_path(&value, "database.port", &[]).unwrap().as_str(),
        Some("5432")
    );
    assert!(Format::Ini.load("[database]\na=1\na=2\n").is_err());
    assert!(Format::Ini.load("[database]\n[database]\n").is_err());
}

#[cfg(feature = "ini")]
#[test]
fn test_ini_fixtures_and_source_editor() {
    let fixture = include_str!("fixtures/ini/core.ini");
    let invalid_fixture = include_str!("fixtures/ini/invalid.ini");
    assert!(Format::Ini.load(fixture).is_ok());
    assert!(Format::Ini.load(invalid_fixture).is_err());
    let source =
        "; comment\r\n[Database]\r\nkey.with.dot = value # literal\r\nempty=\r\n\r\n[empty]\r\n";
    let value = Format::Ini.load(source).unwrap();
    assert_eq!(
        get_path(&value, r"Database.key\.with\.dot", &[])
            .unwrap()
            .as_str(),
        Some("value # literal")
    );
    assert!(Format::Ini.load("root=value\n").is_err());
    assert!(Format::Ini.load("[s]\na: b\n").is_err());
    let edited = agent_first_config::format::ini::set_scalar_preserving(
        source,
        r"Database.key\.with\.dot",
        &Value::String("changed".to_string()),
    )
    .unwrap();
    assert!(edited.contains("; comment\r\n"));
    assert!(edited.contains("key.with.dot = changed\r\n"));
    let removed =
        agent_first_config::format::ini::unset_preserving(&edited, "Database.empty").unwrap();
    assert!(removed.contains("[empty]\r\n"));

    let no_final_newline = "[section]\r\nkey = old";
    let edited = agent_first_config::format::ini::set_scalar_preserving(
        no_final_newline,
        "section.key",
        &Value::String("new".to_string()),
    )
    .unwrap();
    assert_eq!(edited, "[section]\r\nkey = new");
}

#[cfg(feature = "dotenv")]
#[test]
fn test_dotenv_read_semantics() {
    let fixture = include_str!("fixtures/dotenv/core.env");
    assert!(Format::Dotenv.load(fixture).is_ok());
    assert!(
        Format::Dotenv
            .load(include_str!("fixtures/dotenv/invalid.env"))
            .is_err()
    );
    let content = concat!(
        "# comment\r\n",
        " BASIC = value with spaces  # comment\r\n",
        "export EMPTY=\r\n",
        "SINGLE='literal # value'\r\n",
        "DOUBLE=\"line\\nquoted\\t\\\"value\\\"\\\\ # value\" # comment\r\n",
        "NUMBER=5432\r\n",
        "UNICODE=你好\r\n",
        "DUPLICATE=first\r\n",
        "DUPLICATE=last\r\n",
        "REFERENCE=${AFCONFIG_TEST_PROCESS_VALUE}\r\n",
    );
    let error = Format::Dotenv
        .load(content)
        .expect_err("duplicate keys must fail");
    assert!(error.to_string().contains("duplicate"));
    let content = content
        .replace("DUPLICATE=first\r\n", "")
        .replace("DUPLICATE=last\r\n", "");
    let value = Format::Dotenv.load(&content).expect("dotenv should parse");

    let expected = [
        ("BASIC", "value with spaces"),
        ("EMPTY", ""),
        ("SINGLE", "literal # value"),
        ("DOUBLE", "line\nquoted\t\"value\"\\ # value"),
        ("NUMBER", "5432"),
        ("UNICODE", "你好"),
        ("REFERENCE", "${AFCONFIG_TEST_PROCESS_VALUE}"),
    ];
    for (key, expected_value) in expected {
        let actual = get_path(&value, key, &[]).expect("key should exist");
        assert_eq!(actual.as_str(), Some(expected_value));
        assert!(actual.is_string());
    }
}

#[cfg(feature = "dotenv")]
#[test]
fn test_dotenv_multiline_and_missing_set_preserve_source() {
    let source = "# keep\nMULTI=\"first\nsecond\"\nOTHER=abc#def\n";
    let value = Format::Dotenv.load(source).unwrap();
    assert_eq!(
        get_path(&value, "MULTI", &[]).unwrap().as_str(),
        Some("first\nsecond")
    );
    let edited = agent_first_config::format::dotenv::set_scalar_preserving(
        source,
        "NEW",
        &Value::String("value".to_string()),
    )
    .unwrap();
    assert!(edited.starts_with(source));

    let no_final_newline = "export KEY='old'";
    let edited = agent_first_config::format::dotenv::set_scalar_preserving(
        no_final_newline,
        "KEY",
        &Value::String("new value".to_string()),
    )
    .unwrap();
    assert_eq!(edited, "export KEY=\"new value\"");
}

#[cfg(feature = "dotenv")]
#[test]
fn test_dotenv_rejects_invalid_assignments_without_source_text() {
    let error = Format::Dotenv
        .load("SECRET_VALUE_WITHOUT_EQUALS")
        .expect_err("invalid assignment should fail");
    let message = error.to_string();
    assert!(message.contains("line 1"));
    assert!(!message.contains("SECRET_VALUE_WITHOUT_EQUALS"));
}

#[cfg(feature = "dotenv")]
#[test]
fn test_dotenv_save_is_typed_unsupported_operation() {
    let value = Format::Dotenv
        .load("KEY=value\n")
        .expect("dotenv should parse");
    let error = Format::Dotenv
        .save(&value)
        .expect_err("dotenv save should fail");
    assert!(matches!(
        error,
        agent_first_config::ConfigError::UnsupportedOperation { .. }
    ));
}

#[cfg(all(feature = "dotenv", feature = "cli"))]
#[test]
fn test_afconfig_dotenv_get_and_show() {
    use std::process::Command;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = temp_dir.path().join(".env");
    std::fs::write(&config_path, "KEY=value\nEMPTY=\n").expect("failed to write dotenv");

    let get = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .args([config_path.as_os_str(), "get".as_ref(), "KEY".as_ref()])
        .output()
        .expect("failed to run afconfig get");
    assert!(get.status.success());
    let get_json: serde_json::Value =
        serde_json::from_slice(&get.stdout).expect("get output should be JSON");
    assert_eq!(get_json["result"]["value"], "value");

    let show = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .args([config_path.as_os_str(), "show".as_ref()])
        .output()
        .expect("failed to run afconfig show");
    assert!(show.status.success());
    let show_json: serde_json::Value =
        serde_json::from_slice(&show.stdout).expect("show output should be JSON");
    assert_eq!(show_json["result"]["value"]["EMPTY"], "");

    std::fs::write(&config_path, "REFERENCE=${AFCONFIG_TEST_PROCESS_VALUE}\n")
        .expect("failed to replace dotenv");
    let literal = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .env("AFCONFIG_TEST_PROCESS_VALUE", "must-not-be-read")
        .args([
            config_path.as_os_str(),
            "get".as_ref(),
            "REFERENCE".as_ref(),
        ])
        .output()
        .expect("failed to run afconfig literal get");
    assert!(literal.status.success());
    let literal_json: serde_json::Value =
        serde_json::from_slice(&literal.stdout).expect("literal output should be JSON");
    assert_eq!(
        literal_json["result"]["value"],
        "${AFCONFIG_TEST_PROCESS_VALUE}"
    );
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_explicit_format_overrides_extension() {
    use std::process::Command;
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("extensionless.config");
    std::fs::write(&config_path, "{\"name\":\"explicit\"}\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["--format", "json", "get", "name"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["result"]["value"], "explicit");
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_json_keyed_collection_edits_preserve_document() {
    use std::process::Command;
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let source =
        "{\n  \"items\": [\n    {\"id\": \"a\", \"name\": \"A\"}\n  ],\n  \"keep\": 1e+3\n}\n";
    std::fs::write(&config_path, source).unwrap();
    let added = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["add", "items", "b", "--slug-field", "id", "name=B"])
        .output()
        .unwrap();
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stdout)
    );
    let after_add = std::fs::read_to_string(&config_path).unwrap();
    assert!(after_add.contains("1e+3"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&after_add).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let removed = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["remove", "items", "b", "--slug-field", "id"])
        .output()
        .unwrap();
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stdout)
    );
    let after_remove = std::fs::read_to_string(&config_path).unwrap();
    assert!(after_remove.contains("1e+3"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&after_remove).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_keyed_edits_on_nested_dotted_prefix() {
    // A plain (unescaped) nested prefix like `cfg.users` must work end-to-end,
    // not just top-level or backslash-escaped prefixes.
    use std::process::Command;
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let source = "{\n  \"cfg\": {\n    \"users\": [\n      {\"uid\": \"a\", \"role\": \"admin\"}\n    ]\n  }\n}\n";
    std::fs::write(&config_path, source).unwrap();

    let added = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["add", "cfg.users", "bob", "--slug-field", "uid", "role=dev"])
        .output()
        .unwrap();
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stdout)
    );
    let after_add = std::fs::read_to_string(&config_path).unwrap();
    let parsed = serde_json::from_str::<serde_json::Value>(&after_add).unwrap();
    assert_eq!(parsed["cfg"]["users"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["cfg"]["users"][1]["uid"], "bob");
    assert_eq!(parsed["cfg"]["users"][1]["role"], "dev");

    let removed = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["remove", "cfg.users", "a", "--slug-field", "uid"])
        .output()
        .unwrap();
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stdout)
    );
    let after_remove = std::fs::read_to_string(&config_path).unwrap();
    let parsed = serde_json::from_str::<serde_json::Value>(&after_remove).unwrap();
    assert_eq!(parsed["cfg"]["users"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["cfg"]["users"][0]["uid"], "bob");
}

#[cfg(feature = "json")]
#[test]
fn test_json_set_missing_key_inserts_preserving_layout() {
    // A new leaf under an existing object is spliced with sibling indentation;
    // every untouched byte (including number spelling) is preserved.
    let source = "{\n  \"keep\": 1e+3,\n  \"obj\": {\n    \"a\": 1\n  }\n}\n";
    let edited = agent_first_config::format::json::set_scalar_preserving(
        source,
        "obj.b",
        &Value::Integer(2),
    )
    .unwrap();
    assert_eq!(
        edited,
        "{\n  \"keep\": 1e+3,\n  \"obj\": {\n    \"a\": 1,\n    \"b\": 2\n  }\n}\n"
    );
    // Missing intermediate parent fails before producing output.
    assert!(
        agent_first_config::format::json::set_scalar_preserving(
            source,
            "nope.deep",
            &Value::Integer(1)
        )
        .is_err()
    );
}

#[cfg(all(feature = "toml", feature = "cli"))]
#[test]
fn test_afconfig_set_missing_key_toml_and_yaml() {
    use std::process::Command;
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();

    let toml_path = temp_dir.path().join("c.toml");
    std::fs::write(&toml_path, "a = 1\n\n[srv]\nhost = \"x\"  # keep\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&toml_path)
        .args(["set", "srv.port", "8080"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let after = std::fs::read_to_string(&toml_path).unwrap();
    assert!(after.contains("# keep"), "comment preserved: {after}");
    assert!(after.contains("port = 8080"), "new key inserted: {after}");

    #[cfg(feature = "yaml")]
    {
        let yaml_path = temp_dir.path().join("c.yaml");
        std::fs::write(&yaml_path, "a: 1\nsrv:\n  host: x\n").unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .arg(&yaml_path)
            .args(["set", "srv.port", "8080"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        let after = std::fs::read_to_string(&yaml_path).unwrap();
        assert_eq!(after, "a: 1\nsrv:\n  host: x\n  port: 8080\n");
    }
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_value_only_scalar_and_errors() {
    use std::process::Command;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    std::fs::write(
        &config_path,
        "{\"name\":\"hello\",\"empty\":\"\",\"enabled\":true,\"items\":[1]}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .args([config_path.to_str().unwrap(), "get", "name", "--value-only"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"hello");
    assert!(output.stderr.is_empty());

    let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .args([
            config_path.to_str().unwrap(),
            "get",
            "enabled",
            "--value-only",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"true");

    let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .args([
            config_path.to_str().unwrap(),
            "get",
            "items",
            "--value-only",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let response = serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("not a scalar"))
    );
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_secret_sources_and_exact_redaction() {
    use std::process::Command;
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("secrets.json");
    std::fs::write(
        &config_path,
        "{\"password_secret\":\"old\",\"nested\":{\"API_KEY\":\"key\"}}\n",
    )
    .unwrap();
    let set = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "password_secret", "--value-secret", "new-secret"])
        .output()
        .unwrap();
    assert!(set.status.success());
    assert!(!String::from_utf8_lossy(&set.stdout).contains("new-secret"));
    let show = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["--secret-name", "API_KEY", "show"])
        .output()
        .unwrap();
    assert!(show.status.success());
    let response: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(response["result"]["value"]["password_secret"], "***");
    assert_eq!(response["result"]["value"]["nested"]["API_KEY"], "***");
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_secret_round_trips_to_exact_value() {
    // Not leaking is not enough: the *correct* bytes must be stored. Verify
    // through a targeted get (returns the raw value) and from disk.
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("secrets.json");

    std::fs::write(&config_path, "{\"api_key_secret\":\"old\"}\n").unwrap();
    let argv_secret = "s3kr3t-Ünïcode-#=";
    let set = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "api_key_secret", "--value-secret", argv_secret])
        .output()
        .unwrap();
    assert!(set.status.success());
    assert!(!String::from_utf8_lossy(&set.stdout).contains("s3kr3t"));
    let got = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["get", "api_key_secret", "--value-only"])
        .output()
        .unwrap();
    assert!(got.status.success());
    assert_eq!(String::from_utf8_lossy(&got.stdout), argv_secret);
    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(on_disk["api_key_secret"], argv_secret);

    // stdin reads to EOF and preserves the trailing newline exactly.
    let mut child = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "api_key_secret", "--value-secret-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"piped\n").unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    let got = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["get", "api_key_secret", "--value-only"])
        .output()
        .unwrap();
    assert_eq!(got.stdout, b"piped\n");
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_secret_name_redacts_show_but_not_targeted_get() {
    // The whole point of the show/get split: --secret-name protects the
    // non-targeted `show`, but a targeted `get` (and --value-only) still
    // returns the caller-requested raw value.
    use std::process::Command;
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("p.json");
    std::fs::write(&config_path, "{\"PASSWORD\":\"hunter2\"}\n").unwrap();

    let show = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["--secret-name", "PASSWORD", "show"])
        .output()
        .unwrap();
    let show: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(show["result"]["value"]["PASSWORD"], "***");

    let got = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["--secret-name", "PASSWORD", "get", "PASSWORD"])
        .output()
        .unwrap();
    let got: serde_json::Value = serde_json::from_slice(&got.stdout).unwrap();
    assert_eq!(got["result"]["value"], "hunter2");

    let value_only = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args([
            "--secret-name",
            "PASSWORD",
            "get",
            "PASSWORD",
            "--value-only",
        ])
        .output()
        .unwrap();
    assert_eq!(value_only.stdout, b"hunter2");
}

#[cfg(all(feature = "json", feature = "cli", unix))]
#[test]
fn test_afconfig_mutation_preserves_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("m.json");
    std::fs::write(&config_path, "{\"a\":1}\n").unwrap();
    std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o640)).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "a", "2"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let mode = std::fs::metadata(&config_path)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, 0o640,
        "atomic replace must preserve the original mode"
    );
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_protocol_formats_and_parse_errors() {
    use std::process::Command;
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    std::fs::write(&config_path, "{\"name\":\"demo\"}\n").unwrap();
    for output_format in ["yaml", "plain"] {
        let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .arg(&config_path)
            .args(["--output", output_format, "get", "name"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    let invalid = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["unknown-command"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    let response: serde_json::Value = serde_json::from_slice(&invalid.stdout).unwrap();
    assert_eq!(response["error"]["code"], "cli_error");
    assert!(invalid.stderr.is_empty());
}

#[cfg(all(feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_cli_protocol_and_secret_boundaries() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    std::fs::write(
        &config_path,
        "{\"password_secret\":\"old\",\"plain\":\"\",\"whitespace\":\"  x \",\"multi\":\"a\\nb\\n\",\"nested\":{\"API_KEY\":\"old-key\"}}\n",
    )
    .unwrap();

    for args in [
        vec!["set", "password_secret", "--value-secret", "argv-secret"],
        vec!["set", "password_secret", "--value-secret=equals-secret"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .arg(&config_path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            !output
                .stdout
                .windows(11)
                .any(|window| window == b"argv-secret")
        );
        assert!(
            !output
                .stdout
                .windows(13)
                .any(|window| window == b"equals-secret")
        );
        assert!(output.stderr.is_empty());
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "password_secret", "--value-secret-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin-secret\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(
        !output
            .stdout
            .windows(12)
            .any(|window| window == b"stdin-secret")
    );
    assert!(output.stderr.is_empty());

    let mut oversized = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "password_secret", "--value-secret-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    oversized
        .stdin
        .take()
        .unwrap()
        .write_all(&vec![b'x'; 1024 * 1024 + 1])
        .unwrap();
    let oversized_output = oversized.wait_with_output().unwrap();
    assert_eq!(oversized_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&oversized_output.stdout).contains("exceeds"));

    let mut invalid_utf8 = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "password_secret", "--value-secret-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    invalid_utf8
        .stdin
        .take()
        .unwrap()
        .write_all(&[0xff, b'\n'])
        .unwrap();
    let invalid_output = invalid_utf8.wait_with_output().unwrap();
    assert_eq!(invalid_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&invalid_output.stdout).contains("UTF-8"));

    for (key, expected) in [("plain", ""), ("whitespace", "  x "), ("multi", "a\nb\n")] {
        let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .arg(&config_path)
            .args(["get", key, "--value-only"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty());
    }

    let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["--secret-name", "API_KEY", "show"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let show = serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();
    assert_eq!(show["result"]["value"]["password_secret"], "***");
    assert_eq!(show["result"]["value"]["nested"]["API_KEY"], "***");

    // stream_redirect (--stdout-file / --stderr-file) is a Unix-only afdata
    // feature; on non-Unix `install` reports unsupported and afconfig exits
    // non-zero, so exercise the redirect only on Unix.
    #[cfg(unix)]
    {
        let redirected = temp_dir.path().join("raw.out");
        let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .args(["--stdout-file", redirected.to_str().unwrap()])
            .arg(&config_path)
            .args(["get", "multi", "--value-only"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(std::fs::read(redirected).unwrap(), b"a\nb\n");
    }

    for output_format in ["yaml", "plain"] {
        let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .arg(&config_path)
            .args(["--output", output_format, "get", "plain"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    let error = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "plain", "--value-secret", "x", "ordinary"])
        .output()
        .unwrap();
    assert_eq!(error.status.code(), Some(2));
    let error_json = serde_json::from_slice::<serde_json::Value>(&error.stdout).unwrap();
    assert!(error_json["error"].is_object());
    assert!(error.stderr.is_empty());
}

#[cfg(all(feature = "json", feature = "cli", unix))]
#[test]
fn test_afconfig_secret_fd_and_closed_fd_errors() {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    std::fs::write(&config_path, "{\"password_secret\":\"old\"}\n").unwrap();
    let secret_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(secret_file.path(), b"fd-secret\n").unwrap();
    let fd = std::fs::File::open(secret_file.path()).unwrap();
    let source_fd = std::os::unix::io::AsRawFd::as_raw_fd(&fd);
    let child = unsafe {
        Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .arg(&config_path)
            .args(["set", "password_secret", "--value-secret-fd", "42"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .pre_exec(move || {
                if libc::dup2(source_fd, 42) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            })
            .spawn()
            .unwrap()
    };
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let error = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "password_secret", "--value-secret-fd", "2"])
        .output()
        .unwrap();
    assert_eq!(error.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&error.stdout).contains("fd"));

    let error = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&config_path)
        .args(["set", "password_secret", "--value-secret-fd", "1"])
        .output()
        .unwrap();
    assert_eq!(error.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&error.stdout).contains("descriptor >= 3"));
}

#[cfg(all(feature = "dotenv", feature = "cli"))]
#[test]
fn test_afconfig_dotenv_mutations_preserve_source_and_reject_structural_ops() {
    use std::process::Command;
    use tempfile::TempDir;

    let cases: &[&[&str]] = &[
        &["add", "items", "new", "--slug-field", "id", "name=value"],
        &["remove", "items", "old", "--slug-field", "id"],
    ];
    for arguments in cases {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let config_path = temp_dir.path().join(".env");
        let original = "KEY=value\n";
        std::fs::write(&config_path, original).expect("failed to write dotenv");

        let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .arg(&config_path)
            .args(*arguments)
            .output()
            .expect("failed to run afconfig mutation");
        assert_eq!(output.status.code(), Some(1));
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("error output should be JSON");
        assert_eq!(response["error"]["code"], "config_error");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| {
                    message.contains("read-only")
                        || message.contains("does not support")
                        || message.contains("not found")
                })
        );
        assert_eq!(
            std::fs::read_to_string(&config_path).expect("failed to reread dotenv"),
            original
        );
    }

    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = temp_dir.path().join(".env");
    let original = "# keep\nexport KEY=value # comment\nOTHER=unchanged\n";
    std::fs::write(&config_path, original).expect("failed to write dotenv");
    let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .args([
            config_path.as_os_str(),
            "set".as_ref(),
            "KEY".as_ref(),
            "changed".as_ref(),
        ])
        .output()
        .expect("failed to run dotenv set");
    assert!(output.status.success());
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "# keep\nexport KEY=changed # comment\nOTHER=unchanged\n"
    );
}

#[cfg(all(unix, feature = "json", feature = "cli"))]
#[test]
fn test_afconfig_secret_preflight_rejects_hardlink_before_reading_stdin() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let original = temp_dir.path().join("original.json");
    let linked = temp_dir.path().join("linked.json");
    std::fs::write(&original, "{\"token_secret\":\"old\"}\n").unwrap();
    std::fs::hard_link(&original, &linked).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .arg(&linked)
        .args(["set", "token_secret", "--value-secret-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"must-not-be-read\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("hardlinked")
    );
    assert_eq!(
        std::fs::read_to_string(&original).unwrap(),
        "{\"token_secret\":\"old\"}\n"
    );
}

#[cfg(feature = "cli-help")]
#[test]
fn test_afconfig_recursive_markdown_help() {
    use std::process::Command;

    let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .args(["--help", "--recursive", "--output", "markdown"])
        .output()
        .expect("failed to generate afconfig Markdown help");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let markdown = String::from_utf8(output.stdout).expect("help output should be UTF-8");
    assert!(markdown.contains("# Agent-First Config"));
    assert!(markdown.contains("Usage: afconfig"));
    for command in ["get", "set", "add", "remove", "show"] {
        assert!(markdown.contains(&format!("## Agent-First Config {command}")));
    }
    for removed in ["default", "doc", "schema", "implemented:false"] {
        assert!(
            !markdown.contains(removed),
            "stale command surface: {removed}"
        );
    }

    let version = Command::new(env!("CARGO_BIN_EXE_afconfig"))
        .args(["--output", "json", "--version"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_json = serde_json::from_slice::<serde_json::Value>(&version.stdout).unwrap();
    assert_eq!(version_json["result"]["version"], env!("CARGO_PKG_VERSION"));

    for removed in ["default", "doc", "schema"] {
        let output = Command::new(env!("CARGO_BIN_EXE_afconfig"))
            .args(["not-a-file.json", removed])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let error = serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();
        assert!(error["error"].is_object());
    }
}

#[cfg(feature = "json")]
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
