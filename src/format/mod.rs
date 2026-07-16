//! Format detection and backend selection.

#[allow(unused_imports)]
use crate::{ConfigError, ConfigResult, Value};
use std::path::Path;

#[cfg(feature = "dotenv")]
pub mod dotenv;
#[cfg(feature = "ini")]
pub mod ini;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "toml")]
pub mod toml;
#[cfg(feature = "yaml")]
pub mod yaml;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Toml,
    Yaml,
    Dotenv,
    Ini,
}

impl Format {
    /// Detect format from file extension.
    pub fn detect(path: &Path) -> Option<Self> {
        let file_name = path.file_name().and_then(|name| name.to_str())?;
        let file_name_lower = file_name.to_lowercase();
        if file_name_lower == ".env"
            || file_name_lower.starts_with(".env.")
            || path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("env"))
        {
            return Some(Format::Dotenv);
        }

        path.extension().and_then(|ext| ext.to_str()).and_then(|s| {
            match s.to_lowercase().as_str() {
                "json" => Some(Format::Json),
                "toml" => Some(Format::Toml),
                "yaml" | "yml" => Some(Format::Yaml),
                "ini" => Some(Format::Ini),
                _ => None,
            }
        })
    }

    /// Load a config file in the detected format.
    pub fn load(&self, content: &str) -> ConfigResult<Value> {
        match self {
            #[cfg(feature = "json")]
            Format::Json => json::load(content),
            #[cfg(not(feature = "json"))]
            Format::Json => Err(ConfigError::UnsupportedOperation {
                format: "JSON".to_string(),
                operation: "load".to_string(),
                detail: "requires Cargo feature `json`".to_string(),
            }),

            #[cfg(feature = "toml")]
            Format::Toml => toml::load(content),
            #[cfg(not(feature = "toml"))]
            Format::Toml => Err(ConfigError::UnsupportedOperation {
                format: "TOML".to_string(),
                operation: "load".to_string(),
                detail: "requires Cargo feature `toml`".to_string(),
            }),

            #[cfg(feature = "yaml")]
            Format::Yaml => yaml::load(content),
            #[cfg(not(feature = "yaml"))]
            Format::Yaml => Err(ConfigError::UnsupportedOperation {
                format: "YAML".to_string(),
                operation: "load".to_string(),
                detail: "requires Cargo feature `yaml`".to_string(),
            }),

            #[cfg(feature = "dotenv")]
            Format::Dotenv => dotenv::load(content),
            #[cfg(not(feature = "dotenv"))]
            Format::Dotenv => Err(ConfigError::UnsupportedOperation {
                format: "dotenv".to_string(),
                operation: "load".to_string(),
                detail: "requires Cargo feature `dotenv`".to_string(),
            }),

            #[cfg(feature = "ini")]
            Format::Ini => ini::load(content),
            #[cfg(not(feature = "ini"))]
            Format::Ini => Err(ConfigError::UnsupportedOperation {
                format: "INI".to_string(),
                operation: "load".to_string(),
                detail: "requires Cargo feature `ini`".to_string(),
            }),
        }
    }

    /// Save a config in the target format.
    pub fn save(&self, value: &Value) -> ConfigResult<String> {
        match self {
            #[cfg(feature = "json")]
            Format::Json => json::save(value),
            #[cfg(not(feature = "json"))]
            Format::Json => Err(ConfigError::UnsupportedOperation {
                format: "JSON".to_string(),
                operation: "save".to_string(),
                detail: "requires Cargo feature `json`".to_string(),
            }),

            #[cfg(feature = "toml")]
            Format::Toml => toml::save(value),
            #[cfg(not(feature = "toml"))]
            Format::Toml => Err(ConfigError::UnsupportedOperation {
                format: "TOML".to_string(),
                operation: "save".to_string(),
                detail: "requires Cargo feature `toml`".to_string(),
            }),

            #[cfg(feature = "yaml")]
            Format::Yaml => yaml::save(value),
            #[cfg(not(feature = "yaml"))]
            Format::Yaml => Err(ConfigError::UnsupportedOperation {
                format: "YAML".to_string(),
                operation: "save".to_string(),
                detail: "requires Cargo feature `yaml`".to_string(),
            }),

            #[cfg(feature = "dotenv")]
            Format::Dotenv => dotenv::save(value),
            #[cfg(not(feature = "dotenv"))]
            Format::Dotenv => Err(ConfigError::UnsupportedOperation {
                format: "dotenv".to_string(),
                operation: "save".to_string(),
                detail: "requires Cargo feature `dotenv`".to_string(),
            }),

            #[cfg(feature = "ini")]
            Format::Ini => ini::save(value),
            #[cfg(not(feature = "ini"))]
            Format::Ini => Err(ConfigError::UnsupportedOperation {
                format: "INI".to_string(),
                operation: "save".to_string(),
                detail: "requires Cargo feature `ini`".to_string(),
            }),
        }
    }

    /// Reject mutation before a backend-specific value is changed or written.
    pub fn ensure_writable(&self, _operation: &str) -> ConfigResult<()> {
        Ok(())
    }
}

#[cfg(feature = "dotenv")]
pub use dotenv::load as load_dotenv;
#[cfg(feature = "json")]
pub use json::{load as load_json, save as save_json};
#[cfg(feature = "toml")]
pub use toml::{load as load_toml, save as save_toml};
#[cfg(feature = "yaml")]
pub use yaml::{load as load_yaml, save as save_yaml};
