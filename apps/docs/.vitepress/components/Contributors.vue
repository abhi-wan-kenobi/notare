<script setup lang="ts">
import { ref } from "vue";

import { type Contributor } from "../../data/contributors.data.mts";

const props = defineProps<{
  data: Contributor[];
}>();

const STATIC_AVATAR = "https://www.gravatar.com/avatar/?d=mp";

const fallback = (name: string) => {
  return `https://avatar.iran.liara.run/username?username=${
    name.replace(" ", "+")
  }`;
};

const loadedImages = ref<Set<string>>(new Set());

const imageLoaded = (name: string) => {
  loadedImages.value.add(name);
};

const handleClick = (url?: string | null) => {
  if (!url) {
    return;
  }
  window.open(url, "_blank");
};
</script>

<template>
  <div class="flex -space-x-2 rtl:space-x-reverse">
    <button
      v-for="contributor in data"
      :key="contributor.name"
      class="relative"
      @click="handleClick(contributor.html_url)"
    >
      <img
        v-show="!loadedImages.has(contributor.name)"
        class="w-8 h-8 rounded-full"
        :src="STATIC_AVATAR"
        :alt="contributor.name"
        v-tooltip="contributor.name"
        loading="lazy"
      />
      <img
        v-show="loadedImages.has(contributor.name)"
        class="w-8 h-8 rounded-full"
        :src="contributor.avatar_url || fallback(contributor.name)"
        :alt="contributor.name"
        v-tooltip="contributor.name"
        loading="lazy"
        @load="imageLoaded(contributor.name)"
      />
    </button>
  </div>
</template>
