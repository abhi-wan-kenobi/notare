---
title: Analytics
description: Send events to the analytics service
id: analytics
---

# {{ $frontmatter.title }}

**{{ $frontmatter.description }}**

## Notes

- The Typescript bindings exposed by this plugins should be only used in the `desktop` app.

## Commands

```ts-vue
import { commands } from "{{ typedoc.name }}";
```

<PluginCommands :typedoc="typedoc" />

## Resources

<ul>
  <PluginSourceList :id="$frontmatter.id" />
</ul>

<script setup lang="ts">
  import { useData } from "vitepress";
  import { data } from "../data/typedoc.data.mts";
  const { frontmatter } = useData();
  const typedoc = data[frontmatter.value.id];
</script>
