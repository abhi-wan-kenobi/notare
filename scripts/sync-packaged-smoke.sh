#!/usr/bin/env bash
# Smoke-launch a release desktop binary built with `--features sync` (the
# 0.6.8 release configuration) headlessly on Linux, and assert:
#   - the main window mounted (`show_main_window` logged),
#   - no Rust panic and no React `Unrecoverable render error`,
#   - the embedded cr-sqlite engine was extracted and is callable
#     (`cr-sqlite sync engine loaded`, emitted by the db plugin's setup after
#     a `crsql_site_id()` probe) and the degrade path did not fire,
#   - sync stayed OFF on a fresh profile (`sync setting is off`) and no synced
#     table was converted to a CRR (no `*__crsql_clock` table in app.db).
#
# Usage: scripts/sync-packaged-smoke.sh [--skip-build]
# Env:   CARGO_TARGET_DIR (default ~/projects/notare/target)
#        SMOKE_SECONDS    (default 60)
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "sync-packaged-smoke: Linux only" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/projects/notare/target}"
SMOKE_SECONDS="${SMOKE_SECONDS:-60}"
SYNCED_TABLES=(sessions session_documents transcripts action_items tags session_tags)

if [[ "${1:-}" != "--skip-build" ]]; then
  # vite only: `pnpm -F desktop build` also runs a workspace `tsc`, which is a
  # typecheck, not a packaging step. `@hypr/ui` ships its CSS from a
  # gitignored dist/ that a fresh checkout does not have yet.
  (cd "$ROOT" && pnpm -F @hypr/ui build)
  (cd "$ROOT" && pnpm -F desktop exec vite build)
  (cd "$ROOT" && cargo build -p desktop --release --features sync)
fi

BIN="$CARGO_TARGET_DIR/release/desktop"
[[ -x "$BIN" ]] || { echo "sync-packaged-smoke: missing $BIN" >&2; exit 2; }

STUBBIN=/tmp/stubbin
mkdir -p "$STUBBIN"
printf '#!/bin/sh\nexit 0\n' > "$STUBBIN/update-desktop-database"
chmod +x "$STUBBIN/update-desktop-database"

SMOKE_HOME="$(mktemp -d /tmp/notare-sync-smoke.XXXXXX)"
LOG="$SMOKE_HOME/smoke.log"

sherpa_lib="$(find "$CARGO_TARGET_DIR/release" -name 'libsherpa-onnx-c-api.so' -print -quit 2>/dev/null || true)"
ld_path="${LD_LIBRARY_PATH:-}"
if [[ -n "$sherpa_lib" ]]; then
  ld_path="$(dirname "$sherpa_lib")${ld_path:+:$ld_path}"
fi

set +e
(
  cd "$ROOT/apps/desktop/src-tauri"
  env LD_LIBRARY_PATH="$ld_path" PATH="$STUBBIN:$PATH" HOME="$SMOKE_HOME" \
    XDG_DATA_HOME="$SMOKE_HOME/.local/share" XDG_CONFIG_HOME="$SMOKE_HOME/.config" \
    XDG_CACHE_HOME="$SMOKE_HOME/.cache" \
    timeout "$SMOKE_SECONDS" xvfb-run -a "$BIN"
) > "$LOG" 2>&1
status=$?
set -e

count() { grep -c -- "$1" "$LOG" || true; }

failures=0
check() {
  local name="$1" ok="$2"
  if [[ "$ok" == 1 ]]; then
    echo "PASS  $name"
  else
    echo "FAIL  $name"
    failures=$((failures + 1))
  fi
}

shown=$(count show_main_window)
panics=$(count panicked)
render_errors=$(count 'Unrecoverable render error')
engine_loaded=$(count 'cr-sqlite sync engine loaded')
engine_degraded=$(count 'cr-sqlite sync engine failed to load')
engine_broken=$(count 'cr-sqlite sync engine \(extracted but not callable\|load probe failed\|not loaded\)')
sync_off=$(count 'sync setting is off; not starting sync')

echo "exit status: $status (124 = killed by timeout, the expected case)"
echo "log: $LOG"
check "process ran until timeout (exit 124)" "$([[ $status == 124 ]] && echo 1 || echo 0)"
check "show_main_window logged ($shown)" "$([[ $shown -ge 1 ]] && echo 1 || echo 0)"
check "0 panicked ($panics)" "$([[ $panics -eq 0 ]] && echo 1 || echo 0)"
check "0 Unrecoverable render error ($render_errors)" "$([[ $render_errors -eq 0 ]] && echo 1 || echo 0)"
check "cr-sqlite engine loaded ($engine_loaded)" "$([[ $engine_loaded -ge 1 ]] && echo 1 || echo 0)"
check "no engine degrade/probe failure ($((engine_degraded + engine_broken)))" \
  "$([[ $((engine_degraded + engine_broken)) -eq 0 ]] && echo 1 || echo 0)"
check "sync stayed off on a fresh profile ($sync_off)" "$([[ $sync_off -ge 1 ]] && echo 1 || echo 0)"

db="$(find "$SMOKE_HOME/.local/share" -name app.db -print -quit 2>/dev/null || true)"
if [[ -z "$db" ]]; then
  check "app.db created" 0
else
  crr_tables="$(python3 - "$db" "${SYNCED_TABLES[@]}" <<'PY'
import sqlite3, sys
db, tables = sys.argv[1], sys.argv[2:]
con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
names = {row[0] for row in con.execute("SELECT name FROM sqlite_master")}
print(" ".join(t for t in tables if f"{t}__crsql_clock" in names))
PY
)"
  check "no synced table converted to a CRR (${crr_tables:-none})" "$([[ -z "$crr_tables" ]] && echo 1 || echo 0)"
fi

grep -m1 -- 'cr-sqlite sync engine loaded' "$LOG" || true

if [[ $failures -gt 0 ]]; then
  echo "sync-packaged-smoke: $failures check(s) failed; log kept at $LOG" >&2
  exit 1
fi
echo "sync-packaged-smoke: all checks passed"
