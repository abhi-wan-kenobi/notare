---
title: Template
description: Template engine for LLM prompting
id: template
---

# {{ $frontmatter.title }}

**{{ $frontmatter.description }}**

Powered by [minijinja](https://docs.rs/minijinja/latest/minijinja/), with [preserve_order](https://docs.rs/minijinja/latest/minijinja/index.html#optional-features), [json](https://docs.rs/minijinja/latest/minijinja/index.html#optional-features) and [pycompat](https://docs.rs/minijinja-contrib/latest/minijinja_contrib/pycompat/fn.unknown_method_callback.html) enabled.

## Usage

Note that template should be registered before rendering. Usually, it is extension's responsibility to do so.

```ts
await commands.registerTemplate("<TEMPLATE_NAME>", "<TEMPLATE_CONTENT>");
const rendered = await commands.render("<TEMPLATE_NAME>", { a: 1 });
```

## Commands

```ts-vue
import { commands } from "{{ typedoc.name }}";
```

<PluginCommands :typedoc="typedoc" />

## Resources

<ul>
  <PluginSourceList :id="$frontmatter.id" />
  <li><a :href="`https://github.com/fastrepl/hypr/tree/main/plugins/template/src/engine.rs`">Additional template filters and functions</a></li>
</ul>

<script setup lang="ts">
  import { useData } from "vitepress";
  import { data } from "../data/typedoc.data.mts";
  const { frontmatter } = useData();
  const typedoc = data[frontmatter.value.id];
</script>
