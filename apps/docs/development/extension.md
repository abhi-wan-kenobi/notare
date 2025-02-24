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

### How to name

It's really simple.

```
<EXTENSION_NAME>-<ASSET_NAME>
```

For example, the `live-summary` extension has two assets:

- `live-summary-dynamic.gif`
- `live-summary-static.png`

### How to use

To use them, simply use the file name.

```html
<img src="/<FILE_NAME>" />
```

For example:

```html
<img src="/live-summary-dynamic.gif" />
```

## Troubleshooting

### Problem: Storybook add-ons could not be resolved while running `pnpm preview`

Solution: Delete `~/.pnp.cjs` that you find in your home directory. ([more](https://github.com/storybookjs/storybook/issues/20876#issuecomment-1501240993))
