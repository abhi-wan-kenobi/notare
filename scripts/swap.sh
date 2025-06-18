#!/bin/bash

if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "This script only works on macOS"
    exit 1
fi

STABLE_DIR="$HOME/Library/Application Support/com.hyprnote.stable"
NIGHTLY_DIR="$HOME/Library/Application Support/com.hyprnote.nightly"
TEMP_DIR="$HOME/Library/Application Support/com.hyprnote.temp"

cp -r "$STABLE_DIR" ./backup_stable
cp -r "$NIGHTLY_DIR" ./backup_nightly

mv "$STABLE_DIR" "$TEMP_DIR"
mv "$NIGHTLY_DIR" "$STABLE_DIR"
mv "$TEMP_DIR" "$NIGHTLY_DIR"

rm -rf ./backup_stable ./backup_nightly
