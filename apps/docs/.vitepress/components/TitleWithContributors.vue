<script setup lang="ts">
import { data } from "../../data/contributors.data.mts";
import Contributors from "./Contributors.vue";

const props = defineProps<{
  title: string;
  filter?: string;
}>();
</script>

<template>
  <div class="flex items-center justify-between">
    <h1>{{ title }}</h1>
    <Contributors :data="data" />
  </div>
</template>
