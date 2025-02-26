---
title: Emotion Analyzer
description: Analyzes the underlying emotions in expressions, enhancing the quality of hypercharged meeting notes.
source: https://github.com/fastrepl/hypr/tree/main/extensions/emotion-analyzer
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
