<!-- Generated. Do not edit by hand. Regenerate: scripts/projects/agent-first-config/generate-cli-doc.sh -->

# afconfig CLI Reference

## Overview

```
Agent-First Config - Generic dot-path accessor for JSON/TOML/YAML configs, usable both as a **Rust library** and as a standalone **`afconfig` CLI binary**. The library is the primary interface: spores embed it to get `get_key`/`set_key` without writing a dispatch table per field. The CLI wraps the same logic for shell scripts and quick one-off edits from any language.

Usage: afconfig [FILE] [COMMAND]

Commands:
  get      Get a config value by dot-path
  set      Set a config value by dot-path
  add      Add element to keyed list
  remove   Remove element from keyed list
  show     Show full config
  default  Generate default config
  doc      Generate markdown documentation
  schema   Generate JSON Schema
  help     Print this message or the help of the given subcommand(s)

Arguments:
  [FILE]
          Config file path

Options:
  -h, --help
          Print help (see a summary with '-h')
```

## `afconfig get`

```
Get a config value by dot-path

Usage: afconfig get <KEY>

Arguments:
  <KEY>  Dot-separated key path

Options:
  -h, --help  Print help
```

## `afconfig set`

```
Set a config value by dot-path

Usage: afconfig set <KEY> [VALUES]...

Arguments:
  <KEY>        Dot-separated key path
  [VALUES]...  Value(s) to set (multiple args = array)

Options:
  -h, --help  Print help
```

## `afconfig add`

```
Add element to keyed list

Usage: afconfig add <KEY> <SLUG> [FIELD=VALUE]...

Arguments:
  <KEY>             Dot-path to keyed list
  <SLUG>            Slug/ID for new element
  [FIELD=VALUE]...  Field=value pairs

Options:
  -h, --help  Print help
```

## `afconfig remove`

```
Remove element from keyed list

Usage: afconfig remove <KEY> <SLUG>

Arguments:
  <KEY>   Dot-path to keyed list
  <SLUG>  Slug/ID to remove

Options:
  -h, --help  Print help
```

## `afconfig show`

```
Show full config

Usage: afconfig show

Options:
  -h, --help  Print help
```

