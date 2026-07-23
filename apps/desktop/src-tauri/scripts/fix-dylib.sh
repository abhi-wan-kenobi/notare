#!/usr/bin/env bash
set -euo pipefail

# Fixes dynamic library linking in Tauri app bundles.
# Workaround for https://github.com/tauri-apps/tauri/pull/12711
#
# - macOS: rewrites .dylib paths to use @rpath so the app finds bundled libs
# - Linux: sets RPATH to $ORIGIN so the binary finds .so files next to it
# - Windows: no-op (DLLs next to .exe are found automatically)
#
# This runs as Tauri's beforeBundleCommand. Safe to run when no dylibs are present.

OS="$(uname -s)"

case "$OS" in
  Darwin)
    # The beforeBundleCommand hook doesn't pass the bundle dir, so auto-detect
    # the built macOS .app under the cargo target output when $1 is absent.
    BUNDLE_DIR="${1:-}"
    if [ -z "$BUNDLE_DIR" ]; then
      BUNDLE_DIR=$(find target -type d -path "*/bundle/macos" 2>/dev/null | head -n 1) || true
    fi
    if [ -z "$BUNDLE_DIR" ]; then
      echo "[fix-dylib] No bundle dir found, skipping."
      exit 0
    fi

    APP_BUNDLE=$(find "$BUNDLE_DIR" -name "*.app" -maxdepth 1 | head -n 1) || true
    if [ -z "$APP_BUNDLE" ]; then
      echo "[fix-dylib] No .app bundle found, skipping."
      exit 0
    fi

    # Copy build-output dylibs the binary links dynamically (the `ort` crate's
    # ONNX Runtime) into the bundle's Frameworks. Without this the app aborts at
    # launch: "Library not loaded: @rpath/libonnxruntime.<ver>.dylib".
    FRAMEWORKS="$APP_BUNDLE/Contents/Frameworks"
    mkdir -p "$FRAMEWORKS"
    for src in $(find target -name "libonnxruntime*.dylib" 2>/dev/null); do
      echo "[fix-dylib] bundling $(basename "$src")"
      cp -f "$src" "$FRAMEWORKS/"
    done

    DYLIBS=$(find "$FRAMEWORKS" -name "*.dylib" 2>/dev/null)
    if [ -z "$DYLIBS" ]; then
      echo "[fix-dylib] No dylibs found, skipping."
      exit 0
    fi

    BINARY_NAME=$(defaults read "$APP_BUNDLE/Contents/Info.plist" CFBundleExecutable)
    BINARY="$APP_BUNDLE/Contents/MacOS/$BINARY_NAME"

    if [ ! -f "$BINARY" ]; then
      echo "[fix-dylib] Binary not found: $BINARY"
      exit 1
    fi

    echo "[fix-dylib] Fixing dylib paths in: $APP_BUNDLE"

    # Add rpath if not already present
    if ! otool -l "$BINARY" | grep -q "@executable_path/../Frameworks"; then
      install_name_tool -add_rpath "@executable_path/../Frameworks" "$BINARY"
    fi

    for dylib_path in $DYLIBS; do
      dylib=$(basename "$dylib_path")

      # Fix the dylib's own install name
      install_name_tool -id "@rpath/$dylib" "$dylib_path"

      # Rewrite the binary's reference to this dylib (try common prefixes)
      for prefix in /usr/local/lib /opt/homebrew/lib /opt/homebrew/opt/*/lib; do
        install_name_tool -change "$prefix/$dylib" "@rpath/$dylib" "$BINARY" 2>/dev/null || true
      done

      # Also fix inter-dylib references
      for other_path in $DYLIBS; do
        other=$(basename "$other_path")
        if [ "$dylib" != "$other" ]; then
          for prefix in /usr/local/lib /opt/homebrew/lib /opt/homebrew/opt/*/lib; do
            install_name_tool -change "$prefix/$other" "@rpath/$other" "$dylib_path" 2>/dev/null || true
          done
        fi
      done

      echo "[fix-dylib]   Fixed: $dylib"
    done

    echo "[fix-dylib] Done."
    ;;

  Linux)
    # $1 is the cargo release output dir (passed by before-bundle.mjs). The app
    # dynamically links sherpa's libsherpa-onnx-c-api.so + libonnxruntime.so,
    # emitted here by build scripts. The AppImage bundles them via linuxdeploy,
    # but the .deb bundler does NOT — we ship them under /usr/lib/notare/ (see
    # bundle.linux.deb.files) and must fix RPATHs so ld.so resolves them post
    # install (/usr/bin/notare -> $ORIGIN/../lib/notare == /usr/lib/notare).
    RELEASE_DIR="${1:-}"
    if [ -z "$RELEASE_DIR" ] || [ ! -d "$RELEASE_DIR" ]; then
      echo "[fix-dylib] No release dir provided or not found, skipping."
      exit 0
    fi

    if ! command -v patchelf &>/dev/null; then
      echo "[fix-dylib] patchelf not found, skipping."
      exit 0
    fi

    SO_FILES=$(find "$RELEASE_DIR" -maxdepth 1 -name "*.so*" -type f 2>/dev/null)
    if [ -z "$SO_FILES" ]; then
      echo "[fix-dylib] No .so files in $RELEASE_DIR, skipping."
      exit 0
    fi

    # Bundled libs sit together in /usr/lib/notare, so let each resolve its
    # siblings (libsherpa-onnx-c-api.so NEEDs libonnxruntime.so) via $ORIGIN.
    for so in $SO_FILES; do
      echo "[fix-dylib] RPATH \$ORIGIN on $(basename "$so")"
      patchelf --set-rpath '$ORIGIN' "$so" 2>/dev/null || true
    done

    # Point every app executable that links our bundled libs at both the deb's
    # private dir (installed layout) and $ORIGIN (dev/local run + AppImage, where
    # linuxdeploy re-writes RPATH anyway so a missing dir here is harmless).
    for bin in $(find "$RELEASE_DIR" -maxdepth 1 -type f -executable 2>/dev/null); do
      case "$bin" in
        *.so|*.so.*|*.d) continue ;;
      esac
      needed=$(patchelf --print-needed "$bin" 2>/dev/null) || continue
      if echo "$needed" | grep -q 'libsherpa-onnx-c-api\.so\|libonnxruntime\.so'; then
        echo "[fix-dylib] RPATH \$ORIGIN/../lib/notare:\$ORIGIN on $(basename "$bin")"
        patchelf --set-rpath '$ORIGIN/../lib/notare:$ORIGIN' "$bin"
      fi
    done
    echo "[fix-dylib] Done."
    ;;

  *)
    echo "[fix-dylib] Windows or unknown OS, no action needed."
    ;;
esac
