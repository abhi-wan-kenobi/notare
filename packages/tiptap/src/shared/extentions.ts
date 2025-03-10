import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import Placeholder from "@tiptap/extension-placeholder";

import { Mention } from "./mention";

export const extensions = [
  StarterKit.configure({
    code: false,
    codeBlock: false,
    heading: {
      levels: [1],
    },
  }),
  Underline,
  // TODO: darkmode
  Placeholder.configure({
    placeholder: "Take notes about the meeting...",
    emptyEditorClass: "is-editor-empty",
  }),
  Mention({ trigger: "#" }),
];
