# Agent-First Config

Generic dot-path accessor for JSON/TOML/YAML configs, usable both as a **Rust library** and as a standalone **`afconfig` CLI binary**. The library is the primary interface: spores embed it to get `get_key`/`set_key` without writing a dispatch table per field. The CLI wraps the same logic for shell scripts and quick one-off edits from any language.

Neither form requires the other. Add just the lib features you need (`json`, `toml`, `yaml`, `schema`), or install the binary with `--features cli` and never touch Rust at all.

## Install

Rust library:

```bash
cargo add agent-first-config
```

CLI binary (all format backends):

```bash
cargo add agent-first-config --features cli
```

## Core Features

- **Dot-path traversal**: `get("imap.host")`, `set("imap.port", ["993"])` on any serde struct
- **Keyed-list access**: `identities.me.email` routes to the array element where `identity == "me"`
- **Numeric indices**: `steps.0.move_to_mailbox_id` addresses array elements by position
- **Three format backends**: JSON (default), TOML (format-preserving via toml_edit), YAML (re-serializing)
- **Type coercion**: `"true"` → bool, `"993"` → integer, `"[1,2]"` → array; force via `s:`, `b:`, `i:`, `j:` prefixes
- **Seed defaults**: `add_keyed` accepts an optional template object; explicit fields override seed fields
- **Feature-gated deps**: zero format deps by default; enable only what you use

## Lib Usage

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
afconfig get    config.json imap.host
afconfig set    config.json imap.port 993
afconfig add    config.json identities support email=support@example.com name="Support"
afconfig remove config.json identities support
afconfig show   config.json
```

Format is auto-detected from the file extension (`.json`, `.toml`, `.yaml`).

See [`docs/cli.md`](docs/cli.md) for the full CLI reference.

## Traversal Rules

- **Objects**: standard dot-path (`imap.host`)
- **Keyed lists**: slug as path segment (`identities.me`) — register via `KeyedList { prefix, slug_field }`; numeric index takes priority (`steps.0`)
- **Scalar arrays**: multi-value set replaces the whole array (`set tags dev staging prod` → `["dev", "staging", "prod"]`)
- **Greedy key matching**: if the config has a key `"case.add"`, path `actions.case.add.steps` resolves it

## Features

| Feature  | What it enables |
|----------|-----------------|
| `json`   | JSON format (default on) |
| `toml`   | TOML format-preserving via toml_edit |
| `yaml`   | YAML re-serializing (comments not preserved) |
| `schema` | `CliSchema` trait + markdown/annotated-config rendering |
| `cli`    | `afconfig` binary (pulls in clap + agent-first-data; enables all formats + schema) |

No features enabled = zero format deps (traversal and coercion only).

## License

MIT
