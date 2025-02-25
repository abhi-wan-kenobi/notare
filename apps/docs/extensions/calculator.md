---
title: Calculator
description: Conduct simple calculations on the spot.
source: https://github.com/fastrepl/hypr/tree/main/extensions/linear
type: [2x2]
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
