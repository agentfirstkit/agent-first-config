//! Generic dot-path CLI config accessor for JSON/TOML/YAML files.
//!
//! # Features
//!
//! - **json**: Enable JSON format support (default)
//! - **toml**: Enable TOML format support (format-preserving with toml_edit)
//! - **yaml**: Enable YAML format support (re-serialize only, comments not preserved)
//! - **schema**: Enable CliSchema trait and documentation rendering
//! - **cli**: Enable binary (requires clap + agent-first-data)

pub mod coerce;
pub mod error;
pub mod keyed;
pub mod traverse;
pub mod value;

#[cfg(any(feature = "json", feature = "toml", feature = "yaml"))]
pub mod format;

#[cfg(feature = "schema")]
pub mod schema;

pub use coerce::{coerce_scalar, coerce_scalar_typed, coerce_values, coerce_values_typed};
pub use error::{ConfigError, ConfigResult};
pub use keyed::{add_keyed, remove_keyed, KeyedList};
pub use traverse::{add_scalar, get_path, remove_path, remove_scalar, set_path};
pub use value::Value;

#[cfg(any(feature = "json", feature = "toml", feature = "yaml"))]
pub use format::Format;

#[cfg(feature = "schema")]
pub use schema::{
    render_annotated_toml, render_annotated_yaml, render_doc_markdown, CliSchema, FieldDef,
};
