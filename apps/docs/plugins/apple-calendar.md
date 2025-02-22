---
title: Apple Calendar
description: Access to the Apple Calendar on your device
id: apple-calendar
---

# {{ $frontmatter.title }}

**{{ $frontmatter.description }}**

## Notes

- This plugin is only available on `macOS`.
- `Worker` is used to continuously sync the calendar & events to the database.

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
