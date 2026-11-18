#!/usr/bin/env bash
# macOS launch triage for "post setup, the app doesn't open".
# Runs entirely on the affected Mac; needs no sudo. Output ends with a
# VERDICT line that names the failure class and the evidence.
#
# Usage:
#   bash diagnose-macos-launch.sh            # checks installed app + last logs
#   bash diagnose-macos-launch.sh --live     # also launches the app and watches it
#
# Exit codes: 0 = diagnosed (see VERDICT), 1 = no app found, 2 = inconclusive.

set -uo pipefail

APP_NAME="Notare"
BUNDLE_ID="be.abhishek.notare"
DATA_DIR="$HOME/Library/Application Support/notare"
LIVE=0
[[ "${1:-}" == "--live" ]] && LIVE=1

say() { printf '\n== %s ==\n' "$1"; }

# --- locate the app ------------------------------------------------------
say "locate"
CANDIDATES=(
  "/Applications/${APP_NAME}.app"
  "$HOME/Applications/${APP_NAME}.app"
)
APP_PATH=""
for c in "${CANDIDATES[@]}"; do
  if [[ -d "$c" ]]; then APP_PATH="$c"; break; fi
done
mdfind_result=$(mdfind "kMDItemCFBundleIdentifier == '${BUNDLE_ID}'" 2>/dev/null | head -1)
if [[ -z "$APP_PATH" && -n "$mdfind_result" ]]; then
  APP_PATH="$mdfind_result"
fi

if [[ -z "$APP_PATH" ]]; then
  echo "No ${APP_NAME}.app found in /Applications, ~/Applications, or Spotlight (bundle ${BUNDLE_ID})."
  echo "mdfind: ${mdfind_result:-<empty>}"
  exit 1
fi
echo "APP_PATH=$APP_PATH"
BIN_PLIST="$APP_PATH/Contents/Info.plist"
BIN_NAME=$(defaults read "$BIN_PLIST" CFBundleExecutable 2>/dev/null || echo "notare")
BIN="$APP_PATH/Contents/MacOS/$BIN_NAME"
echo "BIN=$BIN"
echo "sw_vers: $(sw_vers -productVersion) ($(uname -m))"

# --- quarantine / gatekeeper ---------------------------------------------
say "quarantine + gatekeeper"
xattr -p com.apple.quarantine "$APP_PATH" 2>/dev/null \
  && QUARANTINED=1 || { QUARANTINED=0; echo "not quarantined"; }
spctl -a -vv "$APP_PATH" 2>&1 || true
codesign -vv --deep "$APP_PATH" 2>&1 | head -5 || true
echo "stapled: $(stapler validate "$APP_PATH" 2>&1 | head -1)"

# --- linkage --------------------------------------------------------------
say "linkage (dyld candidates)"
if [[ -f "$BIN" ]]; then
  # Any NEEDED load command pointing outside the bundle fails at exec.
  otool -L "$BIN" | grep -v \
    -e '^@rpath/' \
    -e '/usr/lib/' \
    -e '/System/' \
    -e "$APP_PATH" || echo "no external dylib references"
  echo "rpaths:"
  otool -l "$BIN" | grep -A2 LC_RPATH | grep path || true
else
  echo "BINARY MISSING: $BIN"
fi

# --- data dir state -------------------------------------------------------
say "app data dir"
if [[ -d "$DATA_DIR" ]]; then
  ls -la "$DATA_DIR" 2>/dev/null | head -12
  [[ -f "$DATA_DIR/global.json" ]] && cat "$DATA_DIR/global.json"
  [[ -f "$DATA_DIR/store.json" ]] && python3 -c '
import json,sys
d=json.load(open(sys.argv[1]))
for scope,val in d.items():
    try:
        inner=json.loads(val) if isinstance(val,str) else val
    except Exception: inner=val
    if isinstance(inner,dict):
        for k,v in inner.items():
            if "onboarding" in k.lower(): print(f"{scope}.{k}=<set>")
' "$DATA_DIR/store.json" 2>/dev/null || true
else
  echo "missing: $DATA_DIR"
fi

# --- crash reports ---------------------------------------------------------
say "recent crash reports"
for f in "$HOME/Library/Logs/DiagnosticReports/"{Notare,notare}"-"*.ips; do
  [[ -f "$f" ]] && { echo "--- $f"; head -40 "$f"; }
done 2>/dev/null
[[ -z "$(ls "$HOME/Library/Logs/DiagnosticReports/" 2>/dev/null | grep -i notare)" ]] \
  && echo "no Notare .ips crash reports"

# --- unified log -----------------------------------------------------------
say "unified log (last notare launches, 30 min)"
LOGS=$(log show --last 30m --predicate \
  'process == "notare" OR eventMessage CONTAINS[c] "notare" OR eventMessage CONTAINS "did_finish_launching"' \
  --style compact 2>/dev/null | grep -v '^Filtering' | grep -iv '^Timestamp' | tail -80)
echo "${LOGS:-<no log lines in last 30m>}"

PANIC=$(echo "$LOGS" | grep -i -e 'panic in a function that cannot unwind' -e 'did_finish_launching' | head -3)
DYLD=$(echo "$LOGS" | grep -i -e 'dyld' -e 'Library not loaded' -e 'code signature' -e 'killed' | head -5)
GATE=$(echo "$LOGS" | grep -i -e 'Gatekeeper' -e 'syspolicyd' -e '-quarantine' | head -5)

# --- live launch (optional) ----------------------------------------------
LAUNCH_EXIT=""
if [[ "$LIVE" == 1 ]]; then
  say "live launch test"
  if pgrep -x "$BIN_NAME" >/dev/null; then
    echo "already running: $(pgrep -x "$BIN_NAME" | tr '\n' ' ')"
    echo "window list:"
    osascript -e 'tell application "System Events" to get name of every window of every process whose name contains "Notare"' 2>/dev/null || true
  else
    BEFORE=$(date +%s)
    open "$APP_PATH"
    for i in $(seq 1 10); do
      sleep 1
      if pgrep -x "$BIN_NAME" >/dev/null; then
        echo "process alive after ${i}s (pid $(pgrep -x "$BIN_NAME"))"
        break
      fi
      [[ $i == 10 ]] && echo "PROCESS DIED within ${i}s of launch"
    done
    sleep 2
    echo "windows:"
    osascript -e 'tell application "System Events" to get name of every window of every process whose name contains "Notare"' 2>/dev/null || true
    LAUNCH_EXIT="observed"
  fi
  say "post-launch log"
  log show --last 2m --predicate \
    'process == "notare"' --style compact 2>/dev/null | tail -40
fi

# --- verdict ---------------------------------------------------------------
say "verdict"
V="INCONCLUSIVE: no distinguishing evidence captured; paste full output back."
if [[ -n "$DYLD" ]]; then
  V="DYLD/LINKAGE: $DYLD"
elif [[ -n "$PANIC" ]]; then
  V="TAO/APP-DELEGATE PANIC: $PANIC"
elif [[ -n "$GATE" ]]; then
  V="GATEKEEPER: $GATE"
elif [[ "$QUARANTINED" == 1 ]]; then
  V="QUARANTINED: app has quarantine xattr; remove it (xattr -dr com.apple.quarantine '$APP_PATH') and relaunch."
fi
echo "VERDICT: $V"
echo "VERDICT_HINT=Also capture: ls $HOME/Library/Logs/DiagnosticReports | tail, and the tail of the unified log above."

case "$V" in
  INCONCLUSIVE:*) exit 2 ;;
  *) exit 0 ;;
esac