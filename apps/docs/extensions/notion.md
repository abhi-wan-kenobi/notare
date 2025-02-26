---
title: Notion
description: View or create Notion pages related to the meeting content.
source: https://github.com/fastrepl/hypr/tree/main/extensions/notion
implemented: false
default: false
cloudOnly: true
plugins: [listener, db]
tags: [knowledge]
---
<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
