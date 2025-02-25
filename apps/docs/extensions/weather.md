---
title: Weather
description: Displays the weather of the configured location.
source: https://github.com/fastrepl/hypr/tree/main/extensions/weather
type: [1x1, 2x1, 2x2]
implemented: false
default: false
cloudOnly: false
plugins: []
tags: [utility]
---
<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
