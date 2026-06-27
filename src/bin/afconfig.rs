//! afconfig CLI binary — dot-path config accessor for JSON/TOML/YAML files.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use agent_first_config::{ConfigError, ConfigResult, Format, KeyedList};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = env!("DISPLAY_NAME"),
    bin_name = "afconfig",
    about = env!("CARGO_PKG_DESCRIPTION"),
    long_about = concat!(env!("DISPLAY_NAME"), " - ", env!("CARGO_PKG_DESCRIPTION")),
)]
struct Args {
    /// Config file path
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Get a config value by dot-path
    Get {
        /// Dot-separated key path
        key: String,
    },
    /// Set a config value by dot-path
    Set {
        /// Dot-separated key path
        key: String,
        /// Value(s) to set (multiple args = array)
        values: Vec<String>,
    },
    /// Add element to keyed list
    Add {
        /// Dot-path to keyed list
        key: String,
        /// Slug/ID for new element
        slug: String,
        /// Field=value pairs
        #[arg(value_name = "FIELD=VALUE")]
        fields: Vec<String>,
    },
    /// Remove element from keyed list
    Remove {
        /// Dot-path to keyed list
        key: String,
        /// Slug/ID to remove
        slug: String,
    },
    /// Show full config
    Show,
    /// Generate default config
    Default,
    /// Generate markdown documentation
    Doc {
        #[arg(long)]
        /// Check against existing file without writing
        dry_run: bool,
    },
    /// Generate JSON Schema
    Schema,
}

fn main() {
    let args = Args::parse();

    if let Err(e) = run(args) {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn run(args: Args) -> ConfigResult<()> {
    let file = args.file.ok_or_else(|| ConfigError::ParseError {
        format: "cli".to_string(),
        detail: "config file path required".to_string(),
    })?;

    let command = args.command.unwrap_or(Command::Show);

    match command {
        Command::Show => show_config(&file)?,
        Command::Get { key } => get_config(&file, &key)?,
        Command::Set { key, values } => set_config(&file, &key, &values)?,
        Command::Add { key, slug, fields } => {
            let field_pairs: Vec<(String, String)> = fields
                .iter()
                .filter_map(|f| {
                    f.split_once('=')
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect();
            add_config(&file, &key, &slug, &field_pairs)?
        }
        Command::Remove { key, slug } => remove_config(&file, &key, &slug)?,
        Command::Default => println!("(default config not yet implemented)"),
        Command::Doc { dry_run: _ } => println!("(doc generation not yet implemented)"),
        Command::Schema => println!("(schema generation not yet implemented)"),
    }

    Ok(())
}

fn show_config(file: &PathBuf) -> ConfigResult<()> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = Format::detect(file).ok_or_else(|| ConfigError::ParseError {
        format: "format".to_string(),
        detail: "cannot detect format from file extension".to_string(),
    })?;

    let value = format.load(&content)?;
    println!("{}", value);

    Ok(())
}

fn get_config(file: &PathBuf, key: &str) -> ConfigResult<()> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = Format::detect(file).ok_or_else(|| ConfigError::ParseError {
        format: "format".to_string(),
        detail: "cannot detect format from file extension".to_string(),
    })?;

    let value = format.load(&content)?;
    let result = agent_first_config::get_path(&value, key, &[])?;

    println!("{}", result);

    Ok(())
}

fn set_config(file: &PathBuf, key: &str, values: &[String]) -> ConfigResult<()> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = Format::detect(file).ok_or_else(|| ConfigError::ParseError {
        format: "format".to_string(),
        detail: "cannot detect format from file extension".to_string(),
    })?;

    let mut value = format.load(&content)?;
    agent_first_config::set_path(&mut value, key, values, &[])?;

    let output = format.save(&value)?;
    std::fs::write(file, output).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    println!("set {} = {:?}", key, values);

    Ok(())
}

fn add_config(
    file: &PathBuf,
    key: &str,
    slug: &str,
    fields: &[(String, String)],
) -> ConfigResult<()> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = Format::detect(file).ok_or_else(|| ConfigError::ParseError {
        format: "format".to_string(),
        detail: "cannot detect format from file extension".to_string(),
    })?;

    let mut value = format.load(&content)?;

    // Stub: keyed_lists would come from user config
    let keyed_lists = [KeyedList {
        prefix: "identities",
        slug_field: "identity",
    }];

    agent_first_config::add_keyed(&mut value, key, slug, &keyed_lists, None, fields)?;

    let output = format.save(&value)?;
    std::fs::write(file, output).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    println!("added {} to {}", slug, key);

    Ok(())
}

fn remove_config(file: &PathBuf, key: &str, slug: &str) -> ConfigResult<()> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = Format::detect(file).ok_or_else(|| ConfigError::ParseError {
        format: "format".to_string(),
        detail: "cannot detect format from file extension".to_string(),
    })?;

    let mut value = format.load(&content)?;

    // Stub: keyed_lists would come from user config
    let keyed_lists = [KeyedList {
        prefix: "identities",
        slug_field: "identity",
    }];

    agent_first_config::remove_keyed(&mut value, key, slug, &keyed_lists)?;

    let output = format.save(&value)?;
    std::fs::write(file, output).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    println!("removed {} from {}", slug, key);

    Ok(())
}
