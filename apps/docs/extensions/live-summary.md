---
title: Summary
description: Summarizes the meeting content
source: https://github.com/fastrepl/hypr/tree/main/extensions/summary
implemented: true
default: true
cloud_only: false
plugins: ["listener", "db"]
tags: [local, realtime, STT, speech-to-text, LLM, AI]
---
<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
