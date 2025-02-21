<script setup lang="ts">
import type { ProjectReflection } from "typedoc";

const props = defineProps<{
  typedoc: ProjectReflection;
}>();

const children: any[] = props.typedoc.children ?? [];

const commands = children
  .find((child) => child.name === "commands")
  .type.declaration.children.map((child: any) => ({
    name: child.name,
    source: child.sources[0],
  }));
</script>

<template>
  <ul>
    <li v-for="command in commands" :key="command.name">
      <a :href="command.source">{{ command.name }}</a>
    </li>
  </ul>
</template>
