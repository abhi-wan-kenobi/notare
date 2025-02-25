---
title: Slack
description: Share the meeting notes with your team
source: https://github.com/fastrepl/hypr/tree/main/extensions/slack
type: [2x2]
implemented: false
default: false
cloudOnly: true
plugins: [listener, db]
tags: [messaging]
---
<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
