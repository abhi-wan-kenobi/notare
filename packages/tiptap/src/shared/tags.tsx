import { ReactRenderer } from "@tiptap/react";
import { PluginKey } from "@tiptap/pm/state";
import { MentionOptions } from "@tiptap/extension-mention";

import tippy from "tippy.js";
import { MentionList, type MentionListRef, type MentionArgs } from "./mention";

export const tagSuggestion = (
  args: MentionArgs,
): MentionOptions["suggestion"] => {
  const { trigger } = args;

  return {
    char: trigger,
    pluginKey: new PluginKey(`mention-${trigger}`),
    items: ({ query }) => {
      return ["TODO1", "TODO2", "TODO3"];
    },
    render: () => {
      let renderer: ReactRenderer<MentionListRef>;
      let popup: ReturnType<typeof tippy>[0];

      return {
        onStart: (props) => {
          if (!props.clientRect) {
            return;
          }

          renderer = new ReactRenderer(MentionList, {
            props,
            editor: props.editor,
          });

          popup = tippy(document.body, {
            getReferenceClientRect: () => props.clientRect?.() || new DOMRect(),
            appendTo: () => document.body,
            content: renderer.element,
            showOnCreate: true,
            interactive: true,
            trigger: "manual",
            placement: "bottom-start",
          });
        },
        onUpdate(props) {
          renderer.updateProps(props);

          if (!props.clientRect) {
            return;
          }

          popup.setProps({
            getReferenceClientRect: () => props.clientRect?.() || new DOMRect(),
          });
        },
        onKeyDown(props) {
          if (props.event.key === "Escape") {
            popup.hide();
            return true;
          }

          return renderer.ref?.onKeyDown(props) ?? false;
        },
        onExit() {
          popup.destroy();
          renderer.destroy();
        },
      };
    },
  };
};
