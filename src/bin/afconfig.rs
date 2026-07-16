//! afconfig CLI binary — dot-path config accessor for JSON/TOML/YAML/dotenv files.

use agent_first_config::{ConfigError, ConfigResult, Format, KeyedList, Value};
use clap::{ArgAction, Parser, Subcommand, error::ErrorKind};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = env!("DISPLAY_NAME"),
    bin_name = "afconfig",
    about = env!("CARGO_PKG_DESCRIPTION"),
    long_about = concat!(env!("DISPLAY_NAME"), " - ", env!("CARGO_PKG_DESCRIPTION")),
    disable_version_flag = true,
)]
struct Args {
    /// Redirect stdout bytes to this file
    #[arg(long = "stdout-file", value_name = "PATH", global = true)]
    stdout_file: Option<PathBuf>,

    /// Redirect stderr bytes to this file
    #[arg(long = "stderr-file", value_name = "PATH", global = true)]
    stderr_file: Option<PathBuf>,

    /// Output format for protocol events
    #[arg(long, value_name = "FORMAT", global = true)]
    output: Option<String>,

    /// Configuration file format override
    #[arg(long = "format", value_name = "FORMAT", global = true)]
    format: Option<String>,

    /// Exact field names to redact in non-targeted output
    #[arg(long = "secret-name", value_name = "FIELD", global = true)]
    secret_names: Vec<String>,

    /// Print the CLI version
    #[arg(short = 'V', long = "version", action = ArgAction::SetTrue)]
    version: bool,

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
        /// Write only the scalar semantic value, without an AFDATA envelope
        #[arg(long = "value-only")]
        value_only: bool,
    },
    /// Set a config value by dot-path
    Set {
        /// Dot-separated key path
        key: String,
        /// Value(s) to set (multiple args = array)
        #[arg(conflicts_with_all = ["value_secret", "value_secret_stdin", "value_secret_prompt", "value_secret_fd"])]
        values: Vec<String>,
        /// Secret scalar value (visible to process observers)
        #[arg(long = "value-secret", conflicts_with_all = ["value_secret_stdin", "value_secret_prompt", "value_secret_fd"])]
        value_secret: Option<String>,
        /// Read one secret scalar from stdin to EOF
        #[arg(long = "value-secret-stdin", conflicts_with_all = ["value_secret", "value_secret_prompt", "value_secret_fd"])]
        value_secret_stdin: bool,
        /// Read one secret scalar from the controlling terminal
        #[arg(long = "value-secret-prompt", conflicts_with_all = ["value_secret", "value_secret_stdin", "value_secret_fd"])]
        value_secret_prompt: bool,
        /// Read one secret scalar from an inherited Unix file descriptor
        #[arg(long = "value-secret-fd", value_name = "FD", conflicts_with_all = ["value_secret", "value_secret_stdin", "value_secret_prompt"])]
        value_secret_fd: Option<String>,
    },
    /// Add element to a keyed list
    Add {
        /// Dot-path to keyed list
        key: String,
        /// Slug/ID for new element
        slug: String,
        /// Field that identifies each element
        #[arg(long = "slug-field")]
        slug_field: String,
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
        /// Field that identifies each element
        #[arg(long = "slug-field")]
        slug_field: String,
    },
    /// Remove one concrete configuration entry
    Unset {
        /// Dot-path to the entry
        key: String,
    },
    /// Show full config
    Show,
}

fn main() {
    let _stream_redirect = install_stream_redirect_or_exit();
    #[cfg(feature = "cli-help")]
    handle_cli_version_or_exit();
    #[cfg(feature = "cli-help")]
    handle_cli_help_or_exit();
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            if matches!(error.kind(), ErrorKind::DisplayHelp) {
                let _ = writeln!(std::io::stdout().lock(), "{error}");
                std::process::exit(0);
            }
            let mut emitter = agent_first_data::CliEmitter::new(
                std::io::stdout().lock(),
                agent_first_data::OutputFormat::Json,
            )
            .with_strict_protocol();
            let _ = emitter.emit_error("cli_error", &error.to_string());
            std::process::exit(2);
        }
    };
    let _stream_redirect_args = (&args.stdout_file, &args.stderr_file);

    let stdout = std::io::stdout();
    let output_format = match parse_output_format(args.output.as_deref().unwrap_or("json")) {
        Ok(format) => format,
        Err(error) => {
            let mut emitter = agent_first_data::CliEmitter::new(
                stdout.lock(),
                agent_first_data::OutputFormat::Json,
            )
            .with_strict_protocol();
            let _ = emitter.emit_error("cli_error", &error);
            std::process::exit(2);
        }
    };
    if args.version {
        if args.output.is_some() {
            let mut emitter = agent_first_data::CliEmitter::new(stdout.lock(), output_format)
                .with_strict_protocol();
            let _ = emitter.emit_validated_value(serde_json::json!({
                "code": "version",
                "version": env!("CARGO_PKG_VERSION")
            }));
            std::process::exit(0);
        }
        if writeln!(stdout.lock(), "{}", env!("CARGO_PKG_VERSION")).is_err() {
            std::process::exit(4);
        }
        std::process::exit(0);
    }
    let output_options = agent_first_data::OutputOptions {
        redaction: agent_first_data::Redactor::new().secret_names(args.secret_names.clone()),
        style: agent_first_data::OutputStyle::default(),
    };
    let mut emitter =
        agent_first_data::CliEmitter::with_options(stdout.lock(), output_format, output_options)
            .with_strict_protocol();
    match run(args) {
        Ok(CliSuccess::Event(result)) => {
            if emitter.emit_result(result).is_err() {
                std::process::exit(4);
            }
        }
        Ok(CliSuccess::Raw(bytes)) => {
            let mut writer = stdout.lock();
            if writer
                .write_all(&bytes)
                .and_then(|_| writer.flush())
                .is_err()
            {
                std::process::exit(4);
            }
        }
        Err(e) => {
            if emitter.emit_error("config_error", &e.to_string()).is_err() {
                std::process::exit(4);
            }
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "cli-help")]
fn handle_cli_version_or_exit() {
    use std::io::Write;

    let raw_args = std::env::args().collect::<Vec<_>>();
    match agent_first_data::cli_handle_version_or_continue(
        &raw_args,
        "afconfig",
        env!("CARGO_PKG_VERSION"),
        &agent_first_data::VersionConfig::conventional_default(),
    ) {
        Ok(Some(version)) => match write!(std::io::stdout(), "{version}") {
            Ok(()) => std::process::exit(0),
            Err(_) => std::process::exit(4),
        },
        Ok(None) => {}
        Err(error) => {
            let rendered =
                agent_first_data::cli_output(&error, agent_first_data::OutputFormat::Json);
            match writeln!(std::io::stdout(), "{rendered}") {
                Ok(()) => std::process::exit(2),
                Err(_) => std::process::exit(4),
            }
        }
    }
}

fn parse_output_format(value: &str) -> Result<agent_first_data::OutputFormat, String> {
    match value.to_ascii_lowercase().as_str() {
        "json" => Ok(agent_first_data::OutputFormat::Json),
        "yaml" => Ok(agent_first_data::OutputFormat::Yaml),
        "plain" | "logfmt" => Ok(agent_first_data::OutputFormat::Plain),
        other => Err(format!(
            "unsupported output format `{other}`; expected json, yaml, or plain"
        )),
    }
}

#[cfg(feature = "cli-help")]
fn handle_cli_help_or_exit() {
    use clap::CommandFactory;
    use std::io::Write;

    let raw_args = std::env::args().collect::<Vec<_>>();
    match agent_first_data::cli_handle_help_or_continue(
        &raw_args,
        &Args::command(),
        &agent_first_data::HelpConfig::human_cli_default().with_protocol_v1(),
    ) {
        Ok(Some(help)) => match write!(std::io::stdout(), "{help}") {
            Ok(()) => std::process::exit(0),
            Err(_) => std::process::exit(4),
        },
        Ok(None) => {}
        Err(error) => {
            let rendered =
                agent_first_data::cli_output(&error, agent_first_data::OutputFormat::Json);
            match writeln!(std::io::stdout(), "{rendered}") {
                Ok(()) => std::process::exit(2),
                Err(_) => std::process::exit(4),
            }
        }
    }
}

fn install_stream_redirect_or_exit()
-> Option<agent_first_data::stream_redirect::InstalledStreamRedirect> {
    match agent_first_data::stream_redirect::install_from_raw_args(std::env::args()) {
        Ok(redirect) => redirect,
        Err(err) => {
            let stdout = std::io::stdout();
            let mut emitter = agent_first_data::CliEmitter::new(
                stdout.lock(),
                agent_first_data::OutputFormat::Json,
            )
            .with_strict_protocol();
            let _ = emitter.emit_error("cli_error", &err.to_string());
            std::process::exit(2);
        }
    }
}

enum CliSuccess {
    Event(serde_json::Value),
    Raw(Vec<u8>),
}

fn run(args: Args) -> ConfigResult<CliSuccess> {
    let output_is_explicit = args.output.is_some();
    let file = args.file.ok_or_else(|| ConfigError::ParseError {
        format: "cli".to_string(),
        detail: "config file path required".to_string(),
    })?;
    let explicit_format = args.format.as_deref();

    let command = args.command.unwrap_or(Command::Show);

    let result = match command {
        Command::Show => CliSuccess::Event(show_config(&file, explicit_format)?),
        Command::Get { key, value_only } => {
            if value_only && output_is_explicit {
                return Err(ConfigError::UnsupportedOperation {
                    format: "cli".to_string(),
                    operation: "get --value-only".to_string(),
                    detail: "--value-only cannot be combined with --output".to_string(),
                });
            }
            let (event, raw) = get_config(&file, explicit_format, &key, value_only)?;
            match raw {
                Some(bytes) => CliSuccess::Raw(bytes),
                None => CliSuccess::Event(event),
            }
        }
        Command::Set {
            key,
            values,
            value_secret,
            value_secret_stdin,
            value_secret_prompt,
            value_secret_fd,
        } => {
            preflight_mutation(&file, "set")?;
            let values = secret_or_values(
                values,
                value_secret,
                value_secret_stdin,
                value_secret_prompt,
                value_secret_fd,
            )?;
            CliSuccess::Event(set_config(&file, explicit_format, &key, &values)?)
        }
        Command::Add {
            key,
            slug,
            slug_field,
            fields,
        } => {
            preflight_mutation(&file, "add")?;
            let mut field_pairs = Vec::with_capacity(fields.len());
            for field in &fields {
                let Some((name, value)) = field.split_once('=') else {
                    return Err(ConfigError::ParseError {
                        format: "cli".to_string(),
                        detail: format!("field `{field}` must use FIELD=VALUE"),
                    });
                };
                if name.is_empty() {
                    return Err(ConfigError::ParseError {
                        format: "cli".to_string(),
                        detail: "field name must not be empty".to_string(),
                    });
                }
                field_pairs.push((name.to_string(), value.to_string()));
            }
            CliSuccess::Event(add_config(
                &file,
                explicit_format,
                &key,
                &slug,
                &slug_field,
                &field_pairs,
            )?)
        }
        Command::Remove {
            key,
            slug,
            slug_field,
        } => {
            preflight_mutation(&file, "remove")?;
            CliSuccess::Event(remove_config(
                &file,
                explicit_format,
                &key,
                &slug,
                &slug_field,
            )?)
        }
        Command::Unset { key } => {
            preflight_mutation(&file, "unset")?;
            CliSuccess::Event(unset_config(&file, explicit_format, &key)?)
        }
    };

    Ok(result)
}

const MAX_VALUE_SECRET_BYTES: usize = 1024 * 1024;

fn preflight_mutation(path: &PathBuf, operation: &str) -> ConfigResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| ConfigError::IoError {
        detail: format!("{operation} preflight `{}`: {error}", path.display()),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(ConfigError::UnsupportedOperation {
            format: "filesystem".to_string(),
            operation: operation.to_string(),
            detail: format!("refusing to mutate symlink `{}`", path.display()),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() > 1 {
            return Err(ConfigError::UnsupportedOperation {
                format: "filesystem".to_string(),
                operation: operation.to_string(),
                detail: format!("refusing to mutate hardlinked file `{}`", path.display()),
            });
        }
    }
    Ok(())
}

fn secret_or_values(
    values: Vec<String>,
    value_secret: Option<String>,
    value_secret_stdin: bool,
    value_secret_prompt: bool,
    value_secret_fd: Option<String>,
) -> ConfigResult<Vec<String>> {
    if let Some(value) = value_secret {
        return Ok(vec![value]);
    }
    if value_secret_stdin {
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            return Err(ConfigError::UnsupportedOperation {
                format: "secret source".to_string(),
                operation: "value-secret-stdin".to_string(),
                detail: "stdin is a TTY; use --value-secret-prompt".to_string(),
            });
        }
        return read_secret_reader(std::io::stdin().lock(), "stdin");
    }
    if value_secret_prompt {
        #[cfg(unix)]
        {
            return read_secret_prompt();
        }
        #[cfg(not(unix))]
        {
            return Err(ConfigError::UnsupportedOperation {
                format: "secret source".to_string(),
                operation: "value-secret-prompt".to_string(),
                detail: "prompt secret input is unsupported on this platform".to_string(),
            });
        }
    }
    if let Some(fd) = value_secret_fd {
        #[cfg(unix)]
        {
            let number = fd.parse::<i32>().map_err(|_| ConfigError::ParseError {
                format: "secret source".to_string(),
                detail: "--value-secret-fd requires a numeric descriptor".to_string(),
            })?;
            if number < 3 {
                return Err(ConfigError::ParseError {
                    format: "secret source".to_string(),
                    detail: "--value-secret-fd requires a descriptor >= 3".to_string(),
                });
            }
            use std::os::unix::io::FromRawFd;
            // SAFETY: ownership is transferred exactly once and the descriptor is closed on drop.
            let file = unsafe { std::fs::File::from_raw_fd(number) };
            return read_secret_reader(file, "file descriptor");
        }
        #[cfg(not(unix))]
        {
            let _ = fd;
            return Err(ConfigError::UnsupportedOperation {
                format: "secret source".to_string(),
                operation: "value-secret-fd".to_string(),
                detail: "raw file descriptors are unsupported on this platform".to_string(),
            });
        }
    }
    Ok(values)
}

fn read_secret_reader<R: std::io::Read>(reader: R, source: &str) -> ConfigResult<Vec<String>> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_VALUE_SECRET_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| ConfigError::IoError {
            detail: format!("read secret from {source}: {error}"),
        })?;
    if bytes.len() > MAX_VALUE_SECRET_BYTES {
        return Err(ConfigError::UnsupportedOperation {
            format: "secret source".to_string(),
            operation: "read".to_string(),
            detail: format!("secret exceeds {MAX_VALUE_SECRET_BYTES} bytes"),
        });
    }
    let value = String::from_utf8(bytes).map_err(|_| ConfigError::ParseError {
        format: "secret source".to_string(),
        detail: "secret input must be valid UTF-8".to_string(),
    })?;
    Ok(vec![value])
}

#[cfg(unix)]
fn read_secret_prompt() -> ConfigResult<Vec<String>> {
    let mut tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|error| ConfigError::IoError {
            detail: format!("open controlling terminal: {error}"),
        })?;
    let status = std::process::Command::new("stty")
        .args(["-echo"])
        .status()
        .map_err(|error| ConfigError::IoError {
            detail: format!("disable terminal echo: {error}"),
        })?;
    if !status.success() {
        return Err(ConfigError::IoError {
            detail: "disable terminal echo failed".to_string(),
        });
    }
    let _echo_guard = TerminalEchoGuard;
    write!(tty, "Secret: ").map_err(|error| ConfigError::IoError {
        detail: format!("write prompt: {error}"),
    })?;
    let mut value = String::new();
    let result = {
        let mut reader = std::io::BufReader::new(&mut tty);
        reader.read_line(&mut value)
    }
    .map_err(|error| ConfigError::IoError {
        detail: format!("read secret from prompt: {error}"),
    });
    let _ = writeln!(tty);
    result?;
    let value = value.trim_end_matches(['\n', '\r']);
    if value.len() > MAX_VALUE_SECRET_BYTES {
        return Err(ConfigError::UnsupportedOperation {
            format: "secret source".to_string(),
            operation: "read".to_string(),
            detail: format!("secret exceeds {MAX_VALUE_SECRET_BYTES} bytes"),
        });
    }
    Ok(vec![value.to_string()])
}

#[cfg(unix)]
struct TerminalEchoGuard;

#[cfg(unix)]
impl Drop for TerminalEchoGuard {
    fn drop(&mut self) {
        let _ = std::process::Command::new("stty").arg("echo").status();
    }
}

fn detect_format(file: &Path, explicit: Option<&str>) -> ConfigResult<Format> {
    if let Some(name) = explicit {
        return match name.to_ascii_lowercase().as_str() {
            "json" => Ok(Format::Json),
            "toml" => Ok(Format::Toml),
            "yaml" | "yml" => Ok(Format::Yaml),
            "dotenv" | "env" => Ok(Format::Dotenv),
            "ini" => Ok(Format::Ini),
            other => Err(ConfigError::ParseError {
                format: "format".to_string(),
                detail: format!("unsupported format `{other}`"),
            }),
        };
    }
    Format::detect(file).ok_or_else(|| ConfigError::ParseError {
        format: "format".to_string(),
        detail: "cannot detect format from file extension; pass --format".to_string(),
    })
}

fn format_name(format: Format) -> &'static str {
    match format {
        Format::Json => "JSON",
        Format::Toml => "TOML",
        Format::Yaml => "YAML",
        Format::Dotenv => "dotenv",
        Format::Ini => "INI",
    }
}

fn show_config(file: &PathBuf, explicit_format: Option<&str>) -> ConfigResult<serde_json::Value> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = detect_format(file, explicit_format)?;

    let value = format.load(&content)?;
    let value = serde_json::Value::from(value);
    Ok(serde_json::json!({"code": "config", "format": format_name(format), "value": value}))
}

fn get_config(
    file: &PathBuf,
    explicit_format: Option<&str>,
    key: &str,
    value_only: bool,
) -> ConfigResult<(serde_json::Value, Option<Vec<u8>>)> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = detect_format(file, explicit_format)?;

    let value = format.load(&content)?;
    let result = agent_first_config::get_path(&value, key, &[])?;
    let raw = if value_only {
        Some(scalar_bytes(&result, key)?)
    } else {
        None
    };

    let result = serde_json::Value::from(result.clone());
    Ok((
        serde_json::json!({"code": "config_value", "format": format_name(format), "key": key, "value": result}),
        raw,
    ))
}

fn scalar_bytes(value: &agent_first_config::Value, key: &str) -> ConfigResult<Vec<u8>> {
    let text = match value {
        agent_first_config::Value::String(value) => return Ok(value.as_bytes().to_vec()),
        agent_first_config::Value::Bool(value) => value.to_string(),
        agent_first_config::Value::Integer(value) => value.to_string(),
        agent_first_config::Value::Unsigned(value) => value.to_string(),
        agent_first_config::Value::Float(value) => {
            if !value.is_finite() {
                return Err(ConfigError::UnsupportedOperation {
                    format: "value".to_string(),
                    operation: "get --value-only".to_string(),
                    detail: format!("non-finite scalar at `{key}`"),
                });
            }
            value.to_string()
        }
        agent_first_config::Value::Null => "null".to_string(),
        agent_first_config::Value::Array(_) | agent_first_config::Value::Object(_) => {
            return Err(ConfigError::UnsupportedOperation {
                format: "value".to_string(),
                operation: "get --value-only".to_string(),
                detail: format!("path `{key}` is not a scalar"),
            });
        }
    };
    Ok(text.into_bytes())
}

fn set_config(
    file: &PathBuf,
    explicit_format: Option<&str>,
    key: &str,
    values: &[String],
) -> ConfigResult<serde_json::Value> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = detect_format(file, explicit_format)?;

    let mut value = format.load(&content)?;
    format.ensure_writable("set")?;
    agent_first_config::set_path(&mut value, key, values, &[])?;

    let target = agent_first_config::get_path(&value, key, &[])?;
    #[allow(unreachable_patterns)]
    let output = match format {
        #[cfg(feature = "toml")]
        Format::Toml => {
            agent_first_config::format::toml::set_scalar_preserving(&content, key, &target)?
        }
        #[cfg(feature = "yaml")]
        Format::Yaml => {
            agent_first_config::format::yaml::set_scalar_preserving(&content, key, &target)?
        }
        #[cfg(feature = "json")]
        Format::Json => {
            agent_first_config::format::json::set_scalar_preserving(&content, key, &target)?
        }
        #[cfg(feature = "dotenv")]
        Format::Dotenv => {
            agent_first_config::format::dotenv::set_scalar_preserving(&content, key, &target)?
        }
        #[cfg(feature = "ini")]
        Format::Ini => {
            agent_first_config::format::ini::set_scalar_preserving(&content, key, &target)?
        }
        _ => format.save(&value)?,
    };
    write_atomic(file, output.as_bytes(), "set")?;

    Ok(serde_json::json!({
        "code": "config_set",
        "format": format_name(format),
        "key": key,
        "write_count": values.len()
    }))
}

#[allow(unreachable_code, unused_variables)]
fn add_config(
    file: &PathBuf,
    explicit_format: Option<&str>,
    key: &str,
    slug: &str,
    slug_field: &str,
    fields: &[(String, String)],
) -> ConfigResult<serde_json::Value> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = detect_format(file, explicit_format)?;

    let mut value = format.load(&content)?;
    format.ensure_writable("add")?;

    let keyed_lists = [KeyedList {
        prefix: key,
        slug_field,
    }];

    agent_first_config::add_keyed(&mut value, key, slug, &keyed_lists, None, fields)?;

    let output: String = match format {
        #[cfg(feature = "json")]
        Format::Json => {
            let array = agent_first_config::get_path(&value, key, &keyed_lists)?;
            let item = array
                .as_array()
                .and_then(|items| items.last())
                .ok_or_else(|| ConfigError::UnsupportedOperation {
                    format: "JSON".to_string(),
                    operation: "add".to_string(),
                    detail: "keyed list did not produce an array item".to_string(),
                })?;
            agent_first_config::format::json::append_array_item_preserving(&content, key, item)?
        }
        #[cfg(feature = "yaml")]
        Format::Yaml => {
            let array = agent_first_config::get_path(&value, key, &keyed_lists)?;
            let item = array
                .as_array()
                .and_then(|items| items.last())
                .ok_or_else(|| ConfigError::UnsupportedOperation {
                    format: "YAML".to_string(),
                    operation: "add".to_string(),
                    detail: "keyed list did not produce an array item".to_string(),
                })?;
            agent_first_config::format::yaml::append_array_item_preserving(&content, key, item)?
        }
        _ => {
            return Err(ConfigError::UnsupportedOperation {
                format: format_name(format).to_string(),
                operation: "add".to_string(),
                detail: "keyed collection source editor is not implemented for this backend"
                    .to_string(),
            });
        }
    };
    write_atomic(file, output.as_bytes(), "add")?;

    Ok(
        serde_json::json!({"code": "config_added", "format": format_name(format), "key": key, "slug": slug}),
    )
}

#[allow(unreachable_code, unused_variables)]
fn remove_config(
    file: &PathBuf,
    explicit_format: Option<&str>,
    key: &str,
    slug: &str,
    slug_field: &str,
) -> ConfigResult<serde_json::Value> {
    let content = std::fs::read_to_string(file).map_err(|e| ConfigError::IoError {
        detail: e.to_string(),
    })?;

    let format = detect_format(file, explicit_format)?;

    let mut value = format.load(&content)?;
    format.ensure_writable("remove")?;

    let keyed_lists = [KeyedList {
        prefix: key,
        slug_field,
    }];

    let original_array = agent_first_config::get_path(&value, key, &keyed_lists)?;
    let removed_index = original_array
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .position(|item| item.get(slug_field).and_then(Value::as_str) == Some(slug))
        })
        .ok_or_else(|| ConfigError::SlugNotFound {
            prefix: key.to_string(),
            slug: slug.to_string(),
        })?;
    #[cfg(not(feature = "yaml"))]
    let _ = removed_index;
    agent_first_config::remove_keyed(&mut value, key, slug, &keyed_lists)?;

    let output: String = match format {
        #[cfg(feature = "json")]
        Format::Json => agent_first_config::format::json::remove_array_item_preserving(
            &content, key, slug, slug_field,
        )?,
        #[cfg(feature = "yaml")]
        Format::Yaml => agent_first_config::format::yaml::remove_array_item_preserving(
            &content,
            key,
            removed_index,
        )?,
        _ => {
            return Err(ConfigError::UnsupportedOperation {
                format: format_name(format).to_string(),
                operation: "remove".to_string(),
                detail: "keyed collection source editor is not implemented for this backend"
                    .to_string(),
            });
        }
    };
    write_atomic(file, output.as_bytes(), "remove")?;

    Ok(
        serde_json::json!({"code": "config_removed", "format": format_name(format), "key": key, "slug": slug}),
    )
}

fn unset_config(
    file: &PathBuf,
    explicit_format: Option<&str>,
    key: &str,
) -> ConfigResult<serde_json::Value> {
    let content = std::fs::read_to_string(file).map_err(|error| ConfigError::IoError {
        detail: format!("unset read `{}`: {error}", file.display()),
    })?;
    let format = detect_format(file, explicit_format)?;
    let mut value = format.load(&content)?;
    format.ensure_writable("unset")?;
    agent_first_config::remove_path(&mut value, key)?;
    #[allow(unreachable_patterns)]
    let output = match format {
        #[cfg(feature = "json")]
        Format::Json => agent_first_config::format::json::unset_preserving(&content, key)?,
        #[cfg(feature = "toml")]
        Format::Toml => agent_first_config::format::toml::unset_preserving(&content, key)?,
        #[cfg(feature = "yaml")]
        Format::Yaml => agent_first_config::format::yaml::unset_preserving(&content, key)?,
        #[cfg(feature = "dotenv")]
        Format::Dotenv => agent_first_config::format::dotenv::unset_preserving(&content, key)?,
        #[cfg(feature = "ini")]
        Format::Ini => agent_first_config::format::ini::unset_preserving(&content, key)?,
        _ => format.save(&value)?,
    };
    write_atomic(file, output.as_bytes(), "unset")?;
    Ok(serde_json::json!({"code": "config_unset", "format": format_name(format), "key": key}))
}

fn write_atomic(path: &PathBuf, bytes: &[u8], operation: &str) -> ConfigResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| ConfigError::IoError {
        detail: format!("{operation} preflight `{}`: {error}", path.display()),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(ConfigError::UnsupportedOperation {
            format: "filesystem".to_string(),
            operation: operation.to_string(),
            detail: format!("refusing to mutate symlink `{}`", path.display()),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() > 1 {
            return Err(ConfigError::UnsupportedOperation {
                format: "filesystem".to_string(),
                operation: operation.to_string(),
                detail: format!("refusing to mutate hardlinked file `{}`", path.display()),
            });
        }
    }

    let parent = path.parent().ok_or_else(|| ConfigError::IoError {
        detail: format!(
            "{operation} has no parent directory for `{}`",
            path.display()
        ),
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ConfigError::IoError {
            detail: format!("{operation} path is not valid UTF-8: `{}`", path.display()),
        })?;
    let pid = std::process::id();
    let mut temp_path = None;
    let mut temp_file = None;
    for attempt in 0..32_u32 {
        let candidate = parent.join(format!(".{file_name}.afconfig.{pid}.{attempt}.tmp"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temp_path = Some(candidate);
                temp_file = Some(file);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(ConfigError::IoError {
                    detail: format!(
                        "{operation} create temporary file in `{}`: {error}",
                        parent.display()
                    ),
                });
            }
        }
    }
    let temp_path = temp_path.ok_or_else(|| ConfigError::IoError {
        detail: format!(
            "{operation} could not allocate temporary file in `{}`",
            parent.display()
        ),
    })?;
    let mut temp_file = temp_file.ok_or_else(|| ConfigError::IoError {
        detail: format!("{operation} temporary file handle missing"),
    })?;
    let result = (|| -> ConfigResult<()> {
        temp_file
            .write_all(bytes)
            .map_err(|error| ConfigError::IoError {
                detail: format!("{operation} write `{}`: {error}", path.display()),
            })?;
        temp_file.sync_all().map_err(|error| ConfigError::IoError {
            detail: format!("{operation} fsync `{}`: {error}", path.display()),
        })?;
        drop(temp_file);
        fs::set_permissions(&temp_path, metadata.permissions()).map_err(|error| {
            ConfigError::IoError {
                detail: format!(
                    "{operation} preserve permissions `{}`: {error}",
                    path.display()
                ),
            }
        })?;
        fs::rename(&temp_path, path).map_err(|error| ConfigError::IoError {
            detail: format!("{operation} atomic replace `{}`: {error}", path.display()),
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}
