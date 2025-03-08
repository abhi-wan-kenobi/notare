---
title: Listener
description: Listen to the meeting and emit events
id: listener
---

# {{ $frontmatter.title }}

**{{ $frontmatter.description }}**

## Notes

- From [extensions](/extensions/index.md) perspective, `subscribe` command is all you need for most cases.
  - <a :href="sessionEvent">SessionEvent</a> will be received to the channel you provided.
  - It's safe to call `subscribe` multiple times, or before `start_session` is called.

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
  const id = frontmatter.value.id;
  const typedoc = data[id];

  const { sources: [s] } = typedoc.children.find((child) => child.name === "SessionEvent");
  const sessionEvent = `https://github.com/fastrepl/hyprnote/blob/main/plugins/${id}/js/${s.fileName}#L${s.line}`;
</script>
