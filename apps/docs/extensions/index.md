<script setup>
import { data } from "../data/extensions.data.mts";
</script>

# Extensions

**Extensions** are add-ons that let you add new features to **Hyprnote**—kind of like how extensions work in VS Code. Extensions are shown in the right side panel of the note.

<Image alt="Extensions" src="/extensions-abstract-light.png" />

## Widgets

**Widgets** are the actual components that you can add to your Hyprnote. They provide specific functionalities or display information, enhancing your note-taking experience. You can think of Widgets as a subset of Extensions.

<Image alt="Widgets" src="/extensions-widgets.png" />

### Types of Widgets

- 1 x 1 widget (160px by 160px)
- 2 x 1 widget (340px by 160px)
- 2 x 2 widget (340px by 340px)
- Full-screen modal

## List of Extensions

<ExtensionsTable :data="data" />

## How easy is it to create an extension?

Since most of the complex logics are extracted as [plugins](/plugins/index.md), `Extension` is small and focused. Also, it is 100% `Typescript` + `React`. So feel free to share your ideas and [contribute new extension](/development/extension).
