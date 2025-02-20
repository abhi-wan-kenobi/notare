<script setup lang="ts">
import { useRouter } from "vitepress";
const props = defineProps({
  title: {
    type: String,
    required: true,
  },
  url: {
    type: String,
    required: true,
  },
  body: {
    type: String,
    required: true,
  },
});

const router = useRouter();
const handleClick = () => {
  router.go(props.url);
};
</script>

<!-- https://github.com/vuejs/vitepress/blob/main/src/client/theme-default/styles/vars.css -->
<template>
  <div
    @click="handleClick"
    class="px-8 rounded-xl bg-[--vp-c-bg-elv] hover:bg-[--vp-c-bg-alt]"
  >
    <h3 class="font-semibold mb-2 text-[--vp-c-text-1]">{{ title }}</h3>
    <p class="text-[--vp-c-text-2]">{{ body }}</p>
  </div>
</template>
