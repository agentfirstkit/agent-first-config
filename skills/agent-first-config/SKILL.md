---
name: agent-first-config
description: Use the agent-first-config Rust library or afconfig CLI to safely read, inspect, and modify JSON, TOML, YAML, dotenv, and INI configuration files, including typed conversion, scalar extraction with get --value-only, secret handling, dot-path escaping, keyed lists, and source-preserving mutation boundaries.
---

# Agent-First Config

Choose the Rust library when configuration behavior belongs inside an
application. Choose `afconfig` for shell scripts and one-off reads or mutations.
Do not use `sed`, regular expressions, temporary parsers, or a generic
serializer to edit a supported file: those approaches can lose comments,
ordering, quoting, anchors, or scalar representation.

## Library workflow

Install only the format backends the application needs and omit CLI
dependencies:

```bash
cargo add agent-first-config --no-default-features --features json,toml
```

Add `yaml`, `dotenv`, `ini`, or `schema` only when the application uses that
backend or API.

Load into the format-neutral `Value`, traverse with the same escaped dot-path
grammar as the CLI, or validate a serde type at the boundary:

```rust
use agent_first_config::{Format, get_path};

let value = Format::Json.load(r#"{"imap":{"host":"mail.example.com"}}"#)?;
let host = get_path(&value, "imap.host", &[])?;
assert_eq!(host.as_str(), Some("mail.example.com"));
# Ok::<(), agent_first_config::ConfigError>(())
```

```rust
use agent_first_config::Format;

#[derive(serde::Deserialize, serde::Serialize)]
struct AppConfig { port: u16, debug: bool }

let value = Format::Json.load(r#"{"port":993,"debug":true}"#)?;
let config: AppConfig = agent_first_config::from_value(&value, "app")?;
let value = agent_first_config::to_value(&config)?;
# Ok::<(), agent_first_config::ConfigError>(())
```

For mutations of existing source, call the selected backend's preserving API
instead of loading and generically reserializing the entire file:

```rust
use agent_first_config::Value;

let source = "# keep\n[database]\nport = 5432 # note\n";
let edited = agent_first_config::format::toml::set_scalar_preserving(
    source,
    "database.port",
    &Value::Integer(3306),
)?;
assert_eq!(edited, "# keep\n[database]\nport = 3306 # note\n");
# Ok::<(), agent_first_config::ConfigError>(())
```

Register every keyed list explicitly with `KeyedList { prefix, slug_field }`.
Use `get_path`, `set_path`, `add_keyed`, and `remove_keyed` for in-memory values;
use backend `*_preserving` functions when source bytes must survive. Propagate
`ConfigError` rather than falling back to a lossy serializer after an
unsupported operation.

## CLI workflow

Use `afconfig` as the first choice for supported configuration-file reads and
writes from a shell.

```bash
cargo install agent-first-config
afconfig config.toml get server.host
afconfig config.toml set server.port 8443
afconfig config.toml show --output yaml
```

### Read workflow

- Confirm the path and format. Extensions are detected automatically; use the
  CLI's explicit format option when available for extensionless files.
- Use `get <KEY>` for a structured AFDATA result and an explicitly requested
  value. Use `show` only for batch inspection; pass every known sensitive legacy
  field as `--secret-name FIELD` (exact field name, no dot path, glob, regex, or
  case folding).
- Use `get <KEY> --value-only` for a scalar needed by a shell script. Assign it
  to a variable first and gate the next command on the exit status; quote the
  variable. Bash command substitution strips trailing newlines and cannot hold
  NUL bytes.

  ```bash
  value=$(afconfig config.json get service.host --value-only) && curl -- "$value"
  ```

  The assignment carries afconfig's exit status, so a failed read short-circuits
  the `&&` and the command is skipped (use an explicit `if`/`else` when a script
  needs a distinct failure branch). Do not inline the substitution as
  `curl -- "$(afconfig … --value-only)"`: on failure that runs the command anyway
  with the error payload as its argument. Never use `eval`, unquoted variables,
  `set -x`, or log a secret variable. Arrays and objects are unsupported in
  `--value-only`; use normal AFDATA `get` for structured data.

### Secret workflow

For `set`, choose exactly one source. Prefer prompt for a human, stdin for a
pipe, and an inherited FD for Unix automation that must keep stdin for another
protocol. `--value-secret VALUE` is convenient but exposes the value to argv,
shell tracing, process listings, and same-user observers. Do not put a secret in
ordinary positional values or environment variables as a recommended workflow.

The four sources are `--value-secret`, `--value-secret-stdin`,
`--value-secret-prompt`, and `--value-secret-fd FD`; each produces one string
scalar and preserves bytes/newlines subject to the CLI's UTF-8 and size limits.
Secret source flags are protected by AFDATA field naming and never belong in
success payloads. A targeted `get` intentionally returns the requested value;
`show`, errors, logs, and startup surfaces must redact `_secret`/`_SECRET` fields
and exact `--secret-name` matches.

### Mutation workflow

- Validate format and path before supplying secret input. Mutation rejects
  symlink and (on Unix) multi-link hardlink targets; after a failure, reread the
  file rather than assuming a partial update.
- Use `set` for an existing scalar or array, `unset <KEY>` for one concrete
  entry, and `add`/`remove` only for keyed lists. Every keyed-list CLI operation
  must pass `--slug-field FIELD`; never let the CLI guess identity fields.
- Escape a literal dot as `\.` and a literal backslash as `\\`. Invalid escapes
  and empty segments are errors. This grammar is shared by get, set, unset,
  scalar operations, and keyed routing.
- INI is INI Core v1: one-level sections, `key=value`, string leaves, exact
  case-sensitive names, and no interpolation, includes, bare keys, duplicate
  sections, or duplicate entries. Dotenv values are strings and `$VAR`/
  `${VAR}` remain literal; unsupported dialect syntax must be reported rather
  than guessed.

All normal CLI output is an AFDATA event. Interpret the result/error object and
exit status together; a zero exit status must not be treated as proof that an
unsupported operation succeeded. Read full flag and output details from
[`docs/cli.md`](docs/cli.md), regenerating it through the repository's fixed
documentation script when the CLI surface changes.

The standalone CLI has no schema input or application type registry. Do not
invoke removed `default`, `doc`, or `schema` commands, and do not assume the
library `CliSchema` automatically controls CLI validation or redaction. When a
backend or source-preserving edit is unsupported, stop and report the structured
error instead of rewriting the file by hand.
