# Hypr Mobile

Expo SDK 55 app for the Rust-first mobile DB bridge in [`crates/mobile-bridge`](../../crates/mobile-bridge).

## Core Flow

- Expo owns the app shell and development build workflow.
- Rust owns DB open/migrate/execute/subscribe and CloudSync calls.
- UniFFI bindings are generated into [`src/generated`](./src/generated) and [`cpp/generated`](./cpp/generated).
- The debug screen lives in [`src/app/index.tsx`](./src/app/index.tsx).

## Commands

```sh
pnpm --dir apps/mobile --ignore-workspace install
pnpm --dir apps/mobile --ignore-workspace typecheck
pnpm --dir apps/mobile --ignore-workspace ios
pnpm --dir apps/mobile --ignore-workspace android
cargo xtask mobile-bridge rn
cargo xtask mobile-bridge ios
cargo xtask mobile-bridge android
```

## Notes

- `expo run:ios` and `expo run:android` are required because the bridge uses custom native code.
- iOS embeds the vendored `CloudSync.xcframework` through the local pod in [`ios/CloudSync`](./ios/CloudSync).
- Android packages the vendored `cloudsync.so` files directly from [`crates/cloudsync/vendor/cloudsync/android`](../../crates/cloudsync/vendor/cloudsync/android).
