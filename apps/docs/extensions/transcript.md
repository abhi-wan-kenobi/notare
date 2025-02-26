---
title: Transcript
description: Converts your meeting content to text in real time and even identifies who’s speaking. If needed, you can manually edit the text and correct speaker names.
source: https://github.com/fastrepl/hypr/tree/main/extensions/transcript
widgets: [2x2]
implemented: true
default: true
cloudOnly: false
plugins: [listener, db]
tags: [local, realtime, STT, speech-to-text, AI]
---
<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
