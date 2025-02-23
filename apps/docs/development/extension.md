# Extension Development

## Quickstart

```bash
todo
```

## Utils

[@hypr/extension-utils](https://github.com/fastrepl/hyprnote/tree/main/extensions/utils) contains essential utilities for extension development.

## Interface

All extension must **default-export** [Extension](https://github.com/fastrepl/hyprnote/blob/main/extensions/types.ts) interface.

## Storybook

```bash
pnpm -F @hypr/<EXTENSION_NAME> preview
```

## Mocking

We use `MSW` for network calls, and `@tauri-apps/api/mocks` for Tauri commands.

## Assets

### Where to store

```lua
<EXTENSION_FOLDER>/assets/<ASSET_NAME>
```

### How to use

Follow this naming convention for assets:

```html
<img src="/<EXTENSION_NAME>/assets/<EXTENSION_NAME>-<ASSET_NAME>" />
```
