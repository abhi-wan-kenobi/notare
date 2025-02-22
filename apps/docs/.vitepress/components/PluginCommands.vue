<script setup lang="ts">
import type { ProjectReflection } from "typedoc";

const props = defineProps<{
  typedoc: ProjectReflection;
}>();

const BASE = "https://github.com/fastrepl/hyprnote";

const name = props.typedoc.name;
const id = name.split("/plugin-")[1];
const children: any[] = props.typedoc.children ?? [];

const commands = children
  .find((child) => child.name === "commands")
  .type.declaration.children.map(({ name, sources: [s] }: any) => ({
    name,
    source: `${BASE}/blob/main/plugins/${id}/js/${s.fileName}#L${s.line}`,
  }));
</script>

<template>
  <ul>
    <li v-for="command in commands" :key="command.name">
      <a :href="command.source">{{ command.name }}</a>
    </li>
  </ul>
</template>
