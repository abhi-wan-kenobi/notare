---
title: Outlook Calendar
description: Outlook calendar integration
source: https://github.com/fastrepl/hypr/tree/main/extensions/outlook-calendar
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

## Notes

- It might be enough to just include `Outlook Calendar` to `Apple Calendar`.

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
