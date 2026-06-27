//! Schema definitions and documentation rendering (feature = "schema").

use crate::Value;

/// Field definition for documentation and schema generation.
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub path: &'static str,
    pub type_name: &'static str,
    pub description: &'static str,
    pub default: Option<&'static str>,
    pub example: Option<&'static str>,
    pub required: bool,
    pub secret: bool,
}

/// Trait for providing config schema and defaults.
pub trait CliSchema {
    fn fields() -> &'static [FieldDef];
    fn default_value() -> Value;
}

/// Render field definitions as markdown reference.
pub fn render_doc_markdown(title: &str, fields: &[FieldDef]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {} Configuration Reference\n\n", title));

    // Group by section (first segment of path)
    let mut sections: std::collections::BTreeMap<&str, Vec<&FieldDef>> =
        std::collections::BTreeMap::new();
    for field in fields {
        let section = field.path.split('.').next().unwrap_or("_");
        sections.entry(section).or_default().push(field);
    }

    for (section, section_fields) in sections {
        out.push_str(&format!("## {}\n\n", section));
        out.push_str("| Key | Type | Default | Description |\n");
        out.push_str("|-----|------|---------|-------------|\n");

        for field in section_fields {
            let key = if field.secret {
                format!("`{}` 🔒", field.path)
            } else {
                format!("`{}`", field.path)
            };

            let type_str = field.type_name;
            let default_str = field.default.unwrap_or("—");
            let required_marker = if field.required { " **Required**" } else { "" };
            let desc = format!("{}{}", field.description, required_marker);

            out.push_str(&format!(
                "| {} | `{}` | `{}` | {} |\n",
                key, type_str, default_str, desc
            ));
        }
        out.push('\n');
    }

    out
}

/// Render annotated TOML with inline comments (if toml_edit feature available).
#[cfg(feature = "json")]
pub fn render_annotated_toml(value: &Value, fields: &[FieldDef]) -> String {
    // Fallback: render as pretty TOML without format-preserving
    // (proper implementation would use toml_edit, requires feature = "toml")
    // For now, just pretty-print the value with inline comments

    let mut out = String::new();
    let field_map: std::collections::HashMap<&str, &FieldDef> =
        fields.iter().map(|f| (f.path, f)).collect();

    render_toml_with_comments(value, &mut out, "", &field_map, 0);
    out
}

#[cfg(not(feature = "json"))]
pub fn render_annotated_toml(_value: &Value, _fields: &[FieldDef]) -> String {
    "# (TOML feature and JSON feature required for annotated output)".to_string()
}

fn render_toml_with_comments(
    value: &Value,
    out: &mut String,
    path: &str,
    field_map: &std::collections::HashMap<&str, &FieldDef>,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);

    if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            let current_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", path, key)
            };

            if let Some(field) = field_map.get(current_path.as_str()) {
                out.push_str(&indent_str);
                out.push_str(&format!("# {}\n", field.description));
                if field.default.is_some() {
                    out.push_str(&indent_str);
                    out.push_str(&format!(
                        "# Type: {} | Default: {}\n",
                        field.type_name,
                        field.default.unwrap_or("—")
                    ));
                }
            }

            out.push_str(&indent_str);
            out.push_str(&format!("{} = ", key));

            match val {
                Value::String(s) => out.push_str(&format!("\"{}\"", s.escape_default())),
                Value::Integer(i) => out.push_str(&i.to_string()),
                Value::Float(f) => out.push_str(&f.to_string()),
                Value::Bool(b) => out.push_str(&b.to_string()),
                Value::Null => out.push_str("null"),
                Value::Array(a) => {
                    out.push('[');
                    for (i, elem) in a.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        match elem {
                            Value::String(s) => {
                                out.push_str(&format!("\"{}\"", s.escape_default()))
                            }
                            Value::Integer(i) => out.push_str(&i.to_string()),
                            _ => {}
                        }
                    }
                    out.push(']');
                }
                Value::Object(_) => {
                    // Skip nested objects in simple render
                }
            }

            out.push('\n');
            if let Some(_field) = field_map.get(current_path.as_str()) {
                out.push('\n');
            }
        }
    }
}

/// Render annotated YAML with inline comments.
#[cfg(feature = "json")]
pub fn render_annotated_yaml(value: &Value, fields: &[FieldDef]) -> String {
    let mut out = String::new();
    let field_map: std::collections::HashMap<&str, &FieldDef> =
        fields.iter().map(|f| (f.path, f)).collect();

    render_yaml_with_comments(value, &mut out, "", &field_map, 0);
    out
}

#[cfg(not(feature = "json"))]
pub fn render_annotated_yaml(_value: &Value, _fields: &[FieldDef]) -> String {
    "# (YAML rendering requires JSON feature)".to_string()
}

fn render_yaml_with_comments(
    value: &Value,
    out: &mut String,
    path: &str,
    field_map: &std::collections::HashMap<&str, &FieldDef>,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);

    if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            let current_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", path, key)
            };

            if let Some(field) = field_map.get(current_path.as_str()) {
                out.push_str(&indent_str);
                out.push_str(&format!("# {}\n", field.description));
                if let Some(default) = field.default {
                    out.push_str(&indent_str);
                    out.push_str(&format!("# Default: {}\n", default));
                }
            }

            out.push_str(&indent_str);
            out.push_str(key);
            out.push_str(": ");

            match val {
                Value::String(s) => out.push_str(&format!("\"{}\"", s.escape_default())),
                Value::Integer(i) => out.push_str(&i.to_string()),
                Value::Float(f) => out.push_str(&f.to_string()),
                Value::Bool(b) => out.push_str(&b.to_string()),
                Value::Null => out.push_str("null"),
                Value::Array(a) => {
                    out.push_str("[\n");
                    for elem in a {
                        out.push_str(&indent_str);
                        out.push_str("  - ");
                        match elem {
                            Value::String(s) => {
                                out.push_str(&format!("\"{}\"", s.escape_default()))
                            }
                            _ => out.push_str(&elem.to_string()),
                        }
                        out.push('\n');
                    }
                    out.push_str(&indent_str);
                    out.push(']');
                }
                Value::Object(_) => {
                    out.push('\n');
                    render_yaml_with_comments(val, out, &current_path, field_map, indent + 2);
                    continue;
                }
            }

            out.push('\n');
        }
    }
}
