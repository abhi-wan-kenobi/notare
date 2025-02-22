<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  id: string;
}>();

const sourceLink = computed(() => {
  return `https://github.com/fastrepl/hypr/tree/main/plugins/${props.id}`;
});
</script>

<template>
  <li><a :href="sourceLink">Github source</a></li>
</template>
