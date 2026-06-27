//! Format detection and backend selection.

#[allow(unused_imports)]
use crate::{ConfigError, ConfigResult, Value};
use std::path::Path;

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
}

impl Format {
    /// Detect format from file extension.
    pub fn detect(path: &Path) -> Option<Self> {
        path.extension().and_then(|ext| ext.to_str()).and_then(|s| {
            match s.to_lowercase().as_str() {
                "json" => Some(Format::Json),
                "toml" => Some(Format::Toml),
                "yaml" | "yml" => Some(Format::Yaml),
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
            Format::Json => Err(ConfigError::ParseError {
                format: "JSON".to_string(),
                detail: "JSON feature not enabled".to_string(),
            }),

            #[cfg(feature = "toml")]
            Format::Toml => toml::load(content),
            #[cfg(not(feature = "toml"))]
            Format::Toml => Err(ConfigError::ParseError {
                format: "TOML".to_string(),
                detail: "TOML feature not enabled".to_string(),
            }),

            #[cfg(feature = "yaml")]
            Format::Yaml => yaml::load(content),
            #[cfg(not(feature = "yaml"))]
            Format::Yaml => Err(ConfigError::ParseError {
                format: "YAML".to_string(),
                detail: "YAML feature not enabled".to_string(),
            }),
        }
    }

    /// Save a config in the target format.
    pub fn save(&self, value: &Value) -> ConfigResult<String> {
        match self {
            #[cfg(feature = "json")]
            Format::Json => json::save(value),
            #[cfg(not(feature = "json"))]
            Format::Json => Err(ConfigError::ParseError {
                format: "JSON".to_string(),
                detail: "JSON feature not enabled".to_string(),
            }),

            #[cfg(feature = "toml")]
            Format::Toml => toml::save(value),
            #[cfg(not(feature = "toml"))]
            Format::Toml => Err(ConfigError::ParseError {
                format: "TOML".to_string(),
                detail: "TOML feature not enabled".to_string(),
            }),

            #[cfg(feature = "yaml")]
            Format::Yaml => yaml::save(value),
            #[cfg(not(feature = "yaml"))]
            Format::Yaml => Err(ConfigError::ParseError {
                format: "YAML".to_string(),
                detail: "YAML feature not enabled".to_string(),
            }),
        }
    }
}

#[cfg(feature = "json")]
pub use json::{load as load_json, save as save_json};
#[cfg(feature = "toml")]
pub use toml::{load as load_toml, save as save_toml};
#[cfg(feature = "yaml")]
pub use yaml::{load as load_yaml, save as save_yaml};
