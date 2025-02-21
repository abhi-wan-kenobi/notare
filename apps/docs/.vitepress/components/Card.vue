<script setup lang="ts">
import { useRouter } from "vitepress";
import { clsx } from "clsx";

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
    :class='
      clsx(
        "cursor-pointer px-8 rounded-xl",
        "bg-[--vp-c-bg-elv] hover:bg-[--vp-c-bg-alt]",
      )
    '
  >
    <h3
      :class='
        clsx(
          "font-semibold text-[--vp-c-text-1]",
          "text-sm sm:text-base md:text-lg",
        )
      '
    >
      {{ title }}
    </h3>
    <p :class='clsx("text-[--vp-c-text-2]", "text-xs sm:text-sm md:text-base")'>
      {{ body }}
    </p>
  </div>
</template>
