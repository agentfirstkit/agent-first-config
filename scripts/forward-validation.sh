#!/usr/bin/env bash

set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
BIN="$ROOT/target/debug/afconfig"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/afconfig-forward.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

[ -x "$BIN" ] || {
    echo "missing built afconfig binary: $BIN" >&2
    exit 1
}

json="$TMP/config.json"
printf '%s\n' '{"items":[{"id":"one","name":"first"}],"password_secret":"old","keep":1e+3}' > "$json"

if value=$("$BIN" "$json" get keep --value-only); then
    [ "$value" = "1000" ]
else
    echo "value-only read failed" >&2
    exit 1
fi
if value=$("$BIN" "$json" get missing --value-only); then
    echo "missing value unexpectedly succeeded: $value" >&2
    exit 1
fi

"$BIN" "$json" add items two --slug-field id name=second >/dev/null
exec 3<<<'forward-secret'
"$BIN" "$json" set password_secret --value-secret-fd 3 >/dev/null
exec 3<&-
show=$("$BIN" --secret-name password_secret "$json" show)
[ "$show" != *forward-secret* ]
case "$show" in
    *'"password_secret":"***"'*) ;;
    *) echo "secret was not redacted" >&2; exit 1 ;;
esac

yaml="$TMP/config.yaml"
printf '%s\n' '# keep' 'server:' '  host: old # note' '  ratio: 1.0' > "$yaml"
"$BIN" "$yaml" set server.host new >/dev/null
grep -F '  host: new # note' "$yaml" >/dev/null

toml="$TMP/config.toml"
printf '%s\n' '# keep' '[server]' 'host = "old" # note' > "$toml"
"$BIN" "$toml" set server.host new >/dev/null
grep -F 'host = "new" # note' "$toml" >/dev/null

dotenv="$TMP/.env"
printf '%s\n' 'TOKEN_SECRET=literal' > "$dotenv"
if value=$("$BIN" "$dotenv" get TOKEN_SECRET --value-only); then
    [ "$value" = literal ]
else
    exit 1
fi

ini="$TMP/config.ini"
printf '%s\n' '[database]' 'host=localhost' > "$ini"
if "$BIN" "$ini" add database ignored --slug-field id name=x >/dev/null 2>&1; then
    echo "INI add unexpectedly succeeded" >&2
    exit 1
fi

big="$TMP/big.json"
printf '{"big":"' > "$big"
dd if=/dev/zero bs=65536 count=1 2>/dev/null | tr '\000' x >> "$big"
printf '"}\n' >> "$big"
set +e
"$BIN" "$big" get big --value-only | head -c 1 >/dev/null
pipe_code="${PIPESTATUS[0]}"
set -e
# Depending on libc/stdout buffering, the producer may observe the closed pipe
# (exit 4) or finish its single write before the reader closes it (exit 0).
[ "$pipe_code" -eq 0 ] || [ "$pipe_code" -eq 4 ]

symlink="$TMP/symlink.json"
ln -s "$json" "$symlink"
if "$BIN" "$symlink" set keep 2 >/dev/null 2>&1; then
    echo "symlink mutation unexpectedly succeeded" >&2
    exit 1
fi

hardlink="$TMP/hardlink.json"
ln "$json" "$hardlink"
if "$BIN" "$hardlink" set keep 2 >/dev/null 2>&1; then
    echo "hardlink mutation unexpectedly succeeded" >&2
    exit 1
fi
