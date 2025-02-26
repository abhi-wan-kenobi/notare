---
title: Google Calendar
description: View or create Google calendar events related to the meeting content.
source: https://github.com/fastrepl/hypr/tree/main/extensions/google-calendar
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

- It might be enough to just include `Google Calendar` to `Apple Calendar`.

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
