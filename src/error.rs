//! Error types with context and helpful hints.

use std::fmt;
use std::io;

pub type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, Clone)]
pub enum ConfigError {
    EmptyPath,
    EmptyValues,
    UnknownSegment {
        path: String,
        segment: String,
    },
    UnregisteredArray {
        path: String,
    },
    SlugNotFound {
        prefix: String,
        slug: String,
    },
    SlugAlreadyExists {
        prefix: String,
        slug: String,
    },
    NotTraversable {
        path: String,
        got: String,
    },
    TypeMismatch {
        path: String,
        expected: String,
        got: String,
        hint: Option<String>,
    },
    PathNotFound {
        path: String,
    },
    IndexOutOfBounds {
        path: String,
        index: usize,
        len: usize,
    },
    ParseError {
        format: String,
        detail: String,
    },
    IoError {
        detail: String,
    },
    UnsupportedOperation {
        format: String,
        operation: String,
        detail: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::EmptyPath => {
                write!(f, "empty path provided")
            }
            ConfigError::EmptyValues => {
                write!(f, "at least one value required")
            }
            ConfigError::UnknownSegment { path, segment } => {
                write!(f, "path `{}` segment `{}` not found", path, segment)
            }
            ConfigError::UnregisteredArray { path } => {
                write!(f, "array at `{}` not registered in KeyedList", path)
            }
            ConfigError::SlugNotFound { prefix, slug } => {
                write!(f, "no element with slug `{}` found in `{}`", slug, prefix)
            }
            ConfigError::SlugAlreadyExists { prefix, slug } => {
                write!(f, "slug `{}` already exists in `{}`", slug, prefix)
            }
            ConfigError::NotTraversable { path, got } => {
                write!(f, "path `{}` is {}, cannot traverse further", path, got)
            }
            ConfigError::TypeMismatch {
                path,
                expected,
                got,
                hint,
            } => {
                write!(f, "field `{}` expects {}, got `{}`", path, expected, got)?;
                if let Some(h) = hint {
                    write!(f, "\n  hint: {}", h)?;
                }
                Ok(())
            }
            ConfigError::PathNotFound { path } => {
                write!(f, "path `{}` not found in config", path)
            }
            ConfigError::IndexOutOfBounds { path, index, len } => {
                write!(
                    f,
                    "index {} out of bounds at `{}` (len {})",
                    index, path, len
                )
            }
            ConfigError::ParseError { format, detail } => {
                write!(f, "failed to parse {}: {}", format, detail)
            }
            ConfigError::IoError { detail } => {
                write!(f, "io error: {}", detail)
            }
            ConfigError::UnsupportedOperation {
                format,
                operation,
                detail,
            } => write!(f, "{} does not support {}: {}", format, operation, detail),
        }
    }
}

impl std::error::Error for ConfigError {}

impl ConfigError {
    /// Wrap a serde deserialization failure as a `TypeMismatch` so callers that
    /// do a read-modify-write cycle (set_path → serde round-trip) surface a
    /// consistent error style rather than a raw serde message.
    pub fn from_serde(path: impl Into<String>, err: impl std::fmt::Display) -> Self {
        let msg = err.to_string();
        // serde messages look like "invalid type: string \"x\", expected u16 at …"
        // Strip the trailing " at line N column M" to keep the hint concise.
        let hint = msg
            .split(" at line ")
            .next()
            .unwrap_or(&msg)
            .trim()
            .to_string();
        ConfigError::TypeMismatch {
            path: path.into(),
            expected: String::new(),
            got: hint,
            hint: None,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::IoError {
            detail: err.to_string(),
        }
    }
}
