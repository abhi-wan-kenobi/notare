---
title: Live Transcript
description: Show a live transcript during the meeting
source: https://github.com/fastrepl/hypr/tree/main/extensions/live-transcript
implemented: true
default: true
cloudOnly: false
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
