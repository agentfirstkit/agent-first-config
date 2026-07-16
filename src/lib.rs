//! Generic dot-path CLI config accessor for JSON/TOML/YAML/dotenv files.
//!
//! # Features
//!
//! - **json**: Enable JSON format support
//! - **toml**: Enable TOML format support (format-preserving with toml_edit)
//! - **yaml**: Enable YAML format support with CST-backed source-preserving mutation
//! - **dotenv**: Enable source-preserving dotenv format support
//! - **ini**: Enable INI Core v1 format support
//! - **schema**: Enable CliSchema trait and documentation rendering
//! - **cli**: Enable binary (requires clap + agent-first-data)
//! - **cli-help**: Enable Agent-First Data recursive and Markdown CLI help

pub mod coerce;
pub mod error;
pub mod keyed;
pub mod path;
pub mod traverse;
pub mod typed;
pub mod value;

#[cfg(any(
    feature = "json",
    feature = "toml",
    feature = "yaml",
    feature = "dotenv",
    feature = "ini"
))]
pub mod format;

#[cfg(feature = "schema")]
pub mod schema;

pub use coerce::{coerce_scalar, coerce_scalar_typed, coerce_values, coerce_values_typed};
pub use error::{ConfigError, ConfigResult};
pub use keyed::{KeyedList, add_keyed, remove_keyed};
pub use path::{join_path, parse_path};
pub use traverse::{add_scalar, get_path, get_path_ref, remove_path, remove_scalar, set_path};
pub use typed::{from_value, to_value};
pub use value::Value;

#[cfg(any(
    feature = "json",
    feature = "toml",
    feature = "yaml",
    feature = "dotenv",
    feature = "ini"
))]
pub use format::Format;

#[cfg(feature = "schema")]
pub use schema::{
    CliSchema, FieldDef, render_annotated_toml, render_annotated_yaml, render_doc_markdown,
};
