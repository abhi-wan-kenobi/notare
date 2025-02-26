---
title: Linear
description: View or create Linear issues related to the meeting content.
source: https://github.com/fastrepl/hypr/tree/main/extensions/linear
implemented: false
default: false
cloud_only: true
plugins: [listener, db]
tags: [ticketing]
---
<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
