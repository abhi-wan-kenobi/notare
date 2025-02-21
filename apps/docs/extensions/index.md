<script setup>
import { data } from "../data/extensions.data.mts";
</script>

# Extensions

`Extension` is pluggable module that can be used to extend the functionality of `Hyprnote`.

Currently, there are **2 types of extensions**:

<Image
  alt="Extensions"
  src="/extensions-abstract-light.png"
  darkSrc="/extensions-abstract-dark.png"
/>

<ExtensionsTable :data="data" />

## How easy is it to create an extension?

Since most of the complex logics are extracted as [plugins](/plugins/index.md), `Extension` is small and focused.
Also, it is 100% `Typescript` + `React`. So feel free to share your ideas and [contribute new extension](/development/extension).
