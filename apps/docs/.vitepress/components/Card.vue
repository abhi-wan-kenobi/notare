<script setup lang="ts">
import { clsx } from "clsx";
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
    :class="
      cn(
        'cursor-pointer px-8 rounded-xl',
        'bg-[--vp-c-bg-elv] hover:bg-[--vp-c-bg-alt]',
      )
    "
  >
    <h3
      :class="
        cn(
          'font-semibold text-[--vp-c-text-1]',
          'text-sm sm:text-base md:text-lg',
        )
      "
    >
      {{ title }}
    </h3>
    <p :class="cn('text-[--vp-c-text-2]', 'text-xs sm:text-sm md:text-base')">
      {{ body }}
    </p>
  </div>
</template>
