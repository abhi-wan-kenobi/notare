---
title: Dino Game
description: A fun game where you control a dinosaur and avoid obstacles.
source: https://github.com/fastrepl/hypr/tree/main/extensions/dino-game
implemented: false
default: false
cloudOnly: false
plugins: []
tags: [game]
---
<TitleWithContributors :title="$frontmatter.title" />

**{{ $frontmatter.description }}**

<ExtensionTags :frontmatter="$frontmatter" />

## Resources

<ul>
  <li><a :href="$frontmatter.source">Github source</a></li>
  <li v-for="plugin in $frontmatter.plugins"><PluginLink :plugin /></li>
</ul>
