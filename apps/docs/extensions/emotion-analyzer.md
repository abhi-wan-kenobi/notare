---
title: Emotion Analyzer
description: Analyzes the underlying emotions in expressions, enhancing the quality of hypercharged meeting notes.
source: https://github.com/fastrepl/hypr/tree/main/extensions/linear
type: [1x1, 2x2, full]
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
