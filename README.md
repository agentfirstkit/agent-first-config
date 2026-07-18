# Agent-First Config — RETIRED

> **⚠️ This project is retired and this repository is archived (read-only).**
>
> `agent-first-config` has been folded into
> **[agent-first-data](https://github.com/agentfirstkit/agent-first-data)**. Its
> document read/edit layer now lives at `agent_first_data::document` (Rust) and in
> the `afdata` CLI (`afdata get` / `value` / `paths` / `keys` / `set` / `unset` /
> `add` / `remove`). No further releases will be made here; the crates.io
> releases are being yanked. Migrate to `agent-first-data`.

---

Generic dot-path accessor for JSON/TOML/YAML/dotenv/INI configs, usable both as a Rust library and as a standalone afconfig CLI binary. The library is the primary interface: spores embed it for generic dot-path access without writing a dispatch table per field. The CLI wraps the same logic for shell scripts and quick one-off reads or edits from any language.

> **Ask your agent:** "Add agent-first-config to my project and use it to read and write my config fields by dot-path."

## Install the Library

With all library features and no CLI dependencies:

```bash
cargo add agent-first-config --no-default-features --features json,toml,yaml,dotenv,ini,schema
```

## Install the CLI

The CLI enables all format backends by default:

```bash
# prebuilt binary
brew install agentfirstkit/tap/afconfig   # macOS / Linux
scoop bucket add agentfirstkit https://github.com/agentfirstkit/scoop-bucket && scoop install afconfig   # Windows

# or from crates.io
cargo install agent-first-config
```

Prebuilt archives are also available from
[GitHub Releases](https://github.com/agentfirstkit/agent-first-config/releases).

## Core Features

- **Dot-path traversal**: `get("imap.host")`, `set("imap.port", ["993"])` on any serde struct
- **Keyed-list access**: `identities.me.email` routes to the array element where `identity == "me"`
- **Numeric indices**: `steps.0.move_to_mailbox_id` addresses array elements by position
- **Five format backends**: JSON, TOML, YAML, dotenv, and INI Core v1
- **Type coercion**: `"true"` → bool, `"993"` → integer, `"[1,2]"` → array; force via `s:`, `b:`, `i:`, `j:` prefixes
- **Seed defaults**: `add_keyed` accepts an optional template object; explicit fields override seed fields
- **Typed adapter**: `from_value`/`to_value` bridge arbitrary serde structs with fallible path-aware errors
- **Source-preserving mutation**: existing entries keep unrelated JSON/TOML/YAML/dotenv/INI source bytes; unsupported CST shapes return structured errors

## Lib Usage

Load a config file and validate it as your own serde struct at the boundary —
`from_value`/`to_value` work on any `Serialize + DeserializeOwned` type and
report errors with the offending path:

```rust
use agent_first_config::Format;

#[derive(serde::Deserialize, serde::Serialize)]
struct AppConfig { port: u16, debug: bool }

let value = Format::Json.load(r#"{"port": 993, "debug": true}"#)?;
let mut typed: AppConfig = agent_first_config::from_value(&value, "app")?;
typed.debug = false;
let back = agent_first_config::to_value(&typed)?; // validated Value, ready to save
```

Or address fields generically by dot-path when you don't have (or want) a typed
struct:

```rust
use agent_first_config::{Value, get_path, set_path, add_keyed, KeyedList};

let mut config = Value::Object(Default::default());

// Set a simple key (type inferred from shape: "993" → integer)
set_path(&mut config, "imap.port", &["993".to_string()], &[])?;

// Add to a keyed list with a seed template
let keyed = [KeyedList { prefix: "identities", slug_field: "identity" }];
let seed = /* Value::Object with default fields */;
add_keyed(&mut config, "identities", "me", &keyed, Some(&seed),
          &[("email".to_string(), "me@example.com".to_string())])?;

// Traverse by slug or numeric index
let email = get_path(&config, "identities.me.email", &keyed)?;
let step0  = get_path(&config, "steps.0.move_to_mailbox_id", &keyed)?;
```

## CLI Usage

```bash
afconfig config.json get imap.host
afconfig config.json set imap.port 993
afconfig config.json add identities support --slug-field identity email=support@example.com name="Support"
afconfig config.json remove identities support --slug-field identity
afconfig config.json unset imap.port
afconfig config.json show
afconfig --format json extensionless.config get imap.host
afconfig .env get DATABASE_URL
afconfig .env show
```

Format is auto-detected from the file name. Dotenv detection covers `.env`, names beginning with `.env.`, and files with an `.env` extension.

### Dotenv semantics

Dotenv reads and writes are deliberately side-effect free. Existing assignments are edited in place through the source-preserving editor; unsupported structural operations are reported as structured `unsupported`.

The parser accepts blank lines, comments, optional `export`, empty values, unquoted/single-quoted/double-quoted values, common quoted escapes, CRLF, UTF-8, and whitespace-delimited inline comments. Duplicate keys are parse errors. Every value is a string. It never executes shell syntax, reads process environment values, expands `$VAR` or `${VAR}`, searches parent directories, combines overlay files, or modifies the process environment. Variable references remain literal text.

INI uses the conservative INI Core v1 dialect: `[section]` plus `key=value`, exact case-sensitive names, string values, comments only at line start, CRLF preservation, and no interpolation, includes, bare keys, duplicate sections, duplicate entries, or multiline extensions. Mutation errors include format, path, and line/column context without including values.

See [`docs/cli.md`](docs/cli.md) for the full CLI reference.

## Traversal Rules

- **Objects**: standard dot-path (`imap.host`)
- **Keyed lists**: slug as path segment (`identities.me`) — register via `KeyedList { prefix, slug_field }`; numeric index takes priority (`steps.0`)
- **Scalar arrays**: multi-value set replaces the whole array (`set tags dev staging prod` → `["dev", "staging", "prod"]`)
- **Escaped keys**: `\.` addresses a dot inside a key and `\\` addresses a backslash; invalid escapes and empty segments are rejected

## Features

| Feature  | What it enables |
|----------|-----------------|
| `json`   | JSON format |
| `toml`   | TOML format-preserving via toml_edit |
| `yaml`   | YAML CST-backed scalar/entry mutation and reads |
| `dotenv` | Dotenv source-preserving reads and entry mutation without expansion or process-environment access |
| `ini`    | INI Core v1 string-valued sections and entries |
| `schema` | `CliSchema` trait + markdown/annotated-config rendering |
| `cli`    | `afconfig` binary (pulls in clap + agent-first-data) |
| `cli-help` | Recursive and Markdown CLI help through agent-first-data; used to generate `docs/cli.md` |

## License

MIT
