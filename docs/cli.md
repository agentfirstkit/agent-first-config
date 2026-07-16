<!-- Generated. Do not edit by hand. Regenerate: afconfig --help --recursive --output markdown -->

# afconfig CLI Reference

# Agent-First Config - Generic dot-path accessor for JSON/TOML/YAML/dotenv/INI configs, usable both as a Rust library and as a standalone afconfig CLI binary. The library is the primary interface: spores embed it for generic dot-path access without writing a dispatch table per field. The CLI wraps the same logic for shell scripts and quick one-off reads or edits from any language.

```text
Usage: afconfig [OPTIONS] [FILE] [COMMAND]

Commands:
  get     Get a config value by dot-path
  set     Set a config value by dot-path
  add     Add element to a keyed list
  remove  Remove element from keyed list
  unset   Remove one concrete configuration entry
  show    Show full config
  help    Print this message or the help of the given subcommand(s)

Arguments:
  [FILE]
          Config file path

Options:
      --stdout-file <PATH>
          Redirect stdout bytes to this file

      --stderr-file <PATH>
          Redirect stderr bytes to this file

      --output <FORMAT>
          Output format for protocol events

      --format <FORMAT>
          Configuration file format override

      --secret-name <FIELD>
          Exact field names to redact in non-targeted output

  -V, --version
          Print the CLI version

  -h, --help
          Print help. Add --recursive to expand every nested subcommand; add --output json|yaml|markdown to render this help in another format.
```

## Agent-First Config get - Get a config value by dot-path

```text
Usage: get [OPTIONS] <KEY>

Arguments:
  <KEY>
          Dot-separated key path

Options:
      --value-only
          Write only the scalar semantic value, without an AFDATA envelope

  -h, --help
          Print help
```

## Agent-First Config set - Set a config value by dot-path

```text
Usage: set [OPTIONS] <KEY> [VALUES]...

Arguments:
  <KEY>
          Dot-separated key path

  [VALUES]...
          Value(s) to set (multiple args = array)

Options:
      --value-secret <VALUE_SECRET>
          Secret scalar value (visible to process observers)

      --value-secret-stdin
          Read one secret scalar from stdin to EOF

      --value-secret-prompt
          Read one secret scalar from the controlling terminal

      --value-secret-fd <FD>
          Read one secret scalar from an inherited Unix file descriptor

  -h, --help
          Print help
```

## Agent-First Config add - Add element to a keyed list

```text
Usage: add --slug-field <SLUG_FIELD> <KEY> <SLUG> [FIELD=VALUE]...

Arguments:
  <KEY>
          Dot-path to keyed list

  <SLUG>
          Slug/ID for new element

  [FIELD=VALUE]...
          Field=value pairs

Options:
      --slug-field <SLUG_FIELD>
          Field that identifies each element

  -h, --help
          Print help
```

## Agent-First Config remove - Remove element from keyed list

```text
Usage: remove --slug-field <SLUG_FIELD> <KEY> <SLUG>

Arguments:
  <KEY>
          Dot-path to keyed list

  <SLUG>
          Slug/ID to remove

Options:
      --slug-field <SLUG_FIELD>
          Field that identifies each element

  -h, --help
          Print help
```

## Agent-First Config unset - Remove one concrete configuration entry

```text
Usage: unset <KEY>

Arguments:
  <KEY>
          Dot-path to the entry

Options:
  -h, --help
          Print help
```

## Agent-First Config show - Show full config

```text
Usage: show

Options:
  -h, --help
          Print help
```
AFDATA: 0.17.3
