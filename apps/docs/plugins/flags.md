---
title: Flags
description: Feature flags for the app
id: flags
---

# {{ $frontmatter.title }}

**{{ $frontmatter.description }}**

## Notes

- Flags are stored with the `store2` plugin.
- This is `local-only` for now.

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
