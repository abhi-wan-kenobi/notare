# @hypr/tiptap

Shared Tiptap extensions and styles used by the web admin blog editor.

> Being deprecated: the desktop app has migrated off Tiptap to react-prosemirror. This package only remains because `apps/web`'s admin blog editor still depends on the shared extension bundle.

## Exports

### Shared extensions (`@hypr/tiptap/shared`)

- StarterKit (bold, italic, strike, code, headings, lists, blockquote, code block, horizontal rule, hard break)
- Tables (resizable), task lists (nestable), images, links, YouTube embeds
- Hashtag highlighting, AI content highlights, search & replace, streaming animation
- Markdown conversion (`json2md` / `md2json`), content validation, clipboard serialization

### Styles (`@hypr/tiptap/styles.css`)

```css
@import "@hypr/tiptap/styles.css";
```

## Utilities

```ts
import {
  json2md,
  md2json,
  isValidTiptapContent,
  parseJsonContent,
  extractHashtags,
  EMPTY_TIPTAP_DOC,
} from "@hypr/tiptap/shared";
```

## License

[MIT](./LICENSE)
