---
title: Google Calendar
description: Create a google calendar event from the meeting
source: https://github.com/fastrepl/hypr/tree/main/extensions/google-calendar
type: [widget]
implemented: false
default: false
cloudOnly: true
plugins: [db]
tags: [calendar]
---

<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
