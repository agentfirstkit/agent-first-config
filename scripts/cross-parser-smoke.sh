#!/usr/bin/env bash

set -euo pipefail

TMP="$(mktemp -d "${TMPDIR:-/tmp}/afconfig-cross-parser.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM
ENV_FILE="$TMP/shared.env"
printf '%s\n' 'SHARED_VALUE=hello' 'SHARED_EMPTY=' > "$ENV_FILE"

if command -v python3 >/dev/null 2>&1; then
    python3 - "$ENV_FILE" <<'PY'
import configparser
import pathlib
import sys

parser = configparser.ConfigParser()
parser.read_string("[section]\nkey=value # literal\n")
assert parser["section"]["key"] == "value # literal"
assert pathlib.Path(sys.argv[1]).read_text().splitlines()[0] == "SHARED_VALUE=hello"
PY
    if python3 -c 'import dotenv' >/dev/null 2>&1; then
        python3 - "$ENV_FILE" <<'PY'
from dotenv import dotenv_values
import sys
values = dotenv_values(sys.argv[1])
assert values["SHARED_VALUE"] == "hello"
assert values["SHARED_EMPTY"] == ""
PY
    else
        echo "python-dotenv unavailable; shared fixture retained for downstream smoke runs"
    fi
else
    echo "python3 unavailable; Python parser smoke skipped"
fi

if command -v node >/dev/null 2>&1 && node --help 2>&1 | grep -F -- '--env-file' >/dev/null; then
    node --env-file="$ENV_FILE" -e 'if (process.env.SHARED_VALUE !== "hello" || process.env.SHARED_EMPTY !== "") process.exit(1)'
else
    echo "Node.js --env-file unavailable; Node parser smoke skipped"
fi

if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    compose="$TMP/compose.yaml"
    printf '%s\n' 'services:' '  app:' '    image: busybox' '    environment:' '      SHARED_VALUE: ${SHARED_VALUE}' > "$compose"
    docker compose --env-file "$ENV_FILE" -f "$compose" config >/dev/null
else
    echo "Docker Compose unavailable; Compose parser smoke skipped"
fi

# The afconfig parser itself remains the authority for the published dialect;
# these checks only exercise the explicitly shared subset with other parsers.
