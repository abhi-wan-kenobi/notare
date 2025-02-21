---
title: Weather
description: Displays weather for all participants
source: https://github.com/fastrepl/hypr/tree/main/extensions/weather
type: [widget]
implemented: false
default: false
cloudOnly: false
plugins: []
tags: [weather]
---

<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
