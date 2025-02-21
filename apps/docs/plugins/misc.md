---
title: Miscellaneous
description: Small utilities that don't fit anywhere else, at least for now
id: misc
---

# {{ $frontmatter.title }}

**{{ $frontmatter.description }}**

## Commands

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
