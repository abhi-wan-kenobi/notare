<script setup>
import { data } from "../data/extensions.data.mts";
</script>

# Extensions

**Extensions** are add-ons that let you add new features to **Hyprnote**—kind of like how extensions work in VS Code.

In **Hyprnote**, these extensions are called **Widgets**. By default, they are **square (340px by 340px)**, but they can also be **smaller (1x1), wider (2x1), or even full-screen modals.**



`<Image alt="Extensions" src="/extensions-abstract-light.png" />`

<ExtensionsTable :data="data" />

## How easy is it to create an extension?

Since most of the complex logics are extracted as [plugins](/plugins/index.md), `Extension` is small and focused.
Also, it is 100% `Typescript` + `React`. So feel free to share your ideas and [contribute new extension](/development/extension).
