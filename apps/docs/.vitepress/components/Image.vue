<script setup>
defineProps({
  src: {
    type: String,
    required: true,
  },
  darkSrc: {
    type: String,
    default: "",
  },
  alt: {
    type: String,
    default: "",
  },
  imageClass: {
    type: String,
    default: "",
  },
});
</script>

<template>
  <div class="container">
    <img v-if="!darkSrc" :src="src" :alt="alt" :class="imageClass">

    <template v-else>
      <img :src="src" :alt="alt" class="light-only" :class="imageClass">
      <img :src="darkSrc" :alt="alt" class="dark-only" :class="imageClass">
    </template>
  </div>
</template>

<style scoped>
.container {
  display: inline-block;
}

html.dark .light-only {
  display: none !important;
}

html:not(.dark) .dark-only {
  display: none !important;
}
</style>
