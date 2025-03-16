import { DefaultTheme } from "vitepress";

const extensions: DefaultTheme.SidebarItem = {
  text: "Extensions",
  items: [
    { text: "What is an extension?", link: "/extensions" },
    {
      text: "List of extensions",
      items: [
        { text: "Calculator", link: "/extensions/calculator" },
        { text: "Dino Game", link: "/extensions/dino-game" },
        { text: "Emotion Analyzer", link: "/extensions/emotion-analyzer" },
        { text: "Google Calendar", link: "/extensions/google-calendar" },
        { text: "Linear", link: "/extensions/linear" },
        { text: "Live Summary", link: "/extensions/live-summary" },
        { text: "Notion", link: "/extensions/notion" },
        { text: "Outlook Calendar", link: "/extensions/outlook-calendar" },
        { text: "Slack", link: "/extensions/slack" },
        { text: "Timer", link: "/extensions/timer" },
        { text: "Transcript", link: "/extensions/transcript" },
        { text: "Weather", link: "/extensions/weather" },
        { text: "World Clocks", link: "/extensions/world-clocks" },
      ].sort((a, b) => a.text.localeCompare(b.text)),
      collapsed: true,
    },
  ],
};

const plugins: DefaultTheme.SidebarItem = {
  text: "Plugins",
  items: [
    { text: "What is a plugin?", link: "/plugins" },
    {
      text: "List of plugins",
      items: [
        { text: "Database", link: "/plugins/db" },
        { text: "Template", link: "/plugins/template" },
        { text: "Listener", link: "/plugins/listener" },
        { text: "Local LLM", link: "/plugins/local-llm" },
        { text: "Local STT", link: "/plugins/local-stt" },
        { text: "Apple Calendar", link: "/plugins/apple-calendar" },
        { text: "Miscellaneous", link: "/plugins/misc" },
        { text: "Windows", link: "/plugins/windows" },
        { text: "Auth", link: "/plugins/auth" },
        { text: "Sound Effects", link: "/plugins/sfx" },
      ].sort((a, b) => a.text.localeCompare(b.text)),
      collapsed: true,
    },
  ],
};

const development: DefaultTheme.SidebarItem = {
  text: "Development",
  collapsed: true,
  items: [
    { text: "Contributing", link: "/development/contributing" },
    { text: "Extension", link: "/development/extension" },
    { text: "Plugin", link: "/development/plugin" },
    { text: "Internalization", link: "/development/i18n" },
    { text: "Miscellaneous", link: "/development/miscellaneous" },
  ],
};

const sidebar: DefaultTheme.SidebarItem[] = [
  {
    text: "Start Here",
    items: [
      {
        text: "Introduction",
        link: "/",
      },
      {
        text: "Quickstart",
        link: "/quickstart",
      },
    ],
  },
  extensions,
  plugins,
  development,
];

export default sidebar;
