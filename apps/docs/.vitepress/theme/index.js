import DefaultTheme from "vitepress/theme";
import FloatingVue from "floating-vue";

import "virtual:uno.css";
import "floating-vue/dist/style.css";
import "./global.css";

import ExtensionsTable from "../components/ExtensionsTable.vue";
import PluginsTable from "../components/PluginsTable.vue";
import ExtensionTags from "../components/ExtensionTags.vue";
import PluginLink from "../components/PluginLink.vue";
import PluginSourceList from "../components/PluginSourceList.vue";
import Contributors from "../components/Contributors.vue";
import TitleWithContributors from "../components/TitleWithContributors.vue";
import Card from "../components/Card.vue";
import Image from "../components/Image.vue";

/** @type {import('vitepress').Theme} */
export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.use(FloatingVue);
    app.component("ExtensionTags", ExtensionTags);
    app.component("ExtensionsTable", ExtensionsTable);
    app.component("PluginsTable", PluginsTable);
    app.component("PluginLink", PluginLink);
    app.component("PluginSourceList", PluginSourceList);
    app.component("Contributors", Contributors);
    app.component("TitleWithContributors", TitleWithContributors);
    app.component("Card", Card);
    app.component("Image", Image);
  },
};
