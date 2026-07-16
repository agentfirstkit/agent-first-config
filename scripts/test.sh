#!/bin/sh

set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
MANIFEST="$ROOT/Cargo.toml"
DOC_TMP=""
cleanup() {
    rm -f "$DOC_TMP"
}
trap cleanup EXIT HUP INT TERM

cargo fmt --manifest-path "$MANIFEST" --all -- --check
cargo check --manifest-path "$MANIFEST" --no-default-features --lib
cargo check --manifest-path "$MANIFEST" --all-features --all-targets
cargo check --manifest-path "$MANIFEST" --no-default-features --features schema --lib

for feature in json toml yaml dotenv ini; do
    cargo check --manifest-path "$MANIFEST" --no-default-features --features "$feature" --lib
    cargo check --manifest-path "$MANIFEST" --no-default-features --features "cli,$feature" --bin afconfig
done

cargo test --manifest-path "$MANIFEST" --no-default-features --features ini --all-targets

cargo check --manifest-path "$MANIFEST" --no-default-features --features cli-help,json --bin afconfig
cargo check --manifest-path "$MANIFEST" --no-default-features --features cli,schema,json --bin afconfig
cargo clippy --manifest-path "$MANIFEST" --all-features --all-targets -- -D warnings
cargo test --manifest-path "$MANIFEST" --all-features --all-targets
"$ROOT/scripts/forward-validation.sh"
"$ROOT/scripts/cross-parser-smoke.sh"

DOC_TMP="$(mktemp)"
{
    printf '%s\n\n' '<!-- Generated. Do not edit by hand. Regenerate: afconfig --help --recursive --output markdown -->' '# afconfig CLI Reference'
    "$ROOT/target/debug/afconfig" --help --recursive --output markdown
} > "$DOC_TMP"
cmp -s "$DOC_TMP" "$ROOT/docs/cli.md"
