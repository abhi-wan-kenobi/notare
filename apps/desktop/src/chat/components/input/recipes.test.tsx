import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import type * as ReactType from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { clearContentMock, editorState, shellState } = vi.hoisted(() => ({
  clearContentMock: vi.fn(),
  editorState: {
    json: undefined as unknown,
    onUpdate: undefined as undefined | ((json: unknown) => void),
    onSubmit: undefined as undefined | (() => void),
    submitShortcut: undefined as undefined | "mod-enter" | "enter",
  },
  shellState: {
    mode: "RightPanelOpen" as
      | "FloatingClosed"
      | "FloatingOpen"
      | "RightPanelOpen",
  },
}));

vi.mock("@hypr/editor/chat", async () => {
  const React = await vi.importActual<typeof ReactType>("react");
  return {
    ChatEditor: React.forwardRef<
      { clearContent: () => void; focus: () => void; getJSON: () => unknown },
      {
        className: string;
        onSubmit: () => void;
        onUpdate: (json: unknown) => void;
        placeholder: (props: {
          node: { type: { name: string } };
          pos: number;
        }) => string;
        submitShortcut?: "mod-enter" | "enter";
      }
    >(function ChatEditor(
      { className, onSubmit, onUpdate, placeholder, submitShortcut },
      ref,
    ) {
      editorState.onSubmit = onSubmit;
      editorState.onUpdate = onUpdate;
      editorState.submitShortcut = submitShortcut;

      React.useImperativeHandle(ref, () => ({
        clearContent: clearContentMock,
        focus: vi.fn(),
        getJSON: () => editorState.json,
      }));

      return (
        <div
          className={className}
          data-placeholder={placeholder({
            node: { type: { name: "paragraph" } },
            pos: 0,
          })}
          data-testid="chat-editor"
        />
      );
    }),
  };
});

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: shellState.mode,
    },
  }),
}));

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: true,
    elevatedSurfaceClassName: "bg-card text-card-foreground border-border",
    inputEditorClassName: "chat-input-editor text-card-foreground",
    sendButtonDisabledClassName:
      "cursor-default border-border text-muted-foreground/60",
    sendButtonShortcutDisabledClassName: "text-muted-foreground/60",
  }),
}));

vi.mock("~/editor-bridge/mention-config", () => ({
  useMentionConfig: () => undefined,
}));

import { ChatMessageInput } from "./index";

describe("ChatMessageInput Recipe Slash Commands", () => {
  beforeEach(() => {
    cleanup();
    clearContentMock.mockClear();
    editorState.json = { type: "doc", content: [] };
    editorState.onSubmit = undefined;
    editorState.onUpdate = undefined;
    editorState.submitShortcut = undefined;
    shellState.mode = "RightPanelOpen";
  });

  it("opens slash command menu when typing '/' and lists built-in recipes", () => {
    render(
      <ChatMessageInput
        draftKey="recipe-test-session-1"
        onSendMessage={vi.fn()}
      />,
    );

    // Initial state: menu is not visible
    expect(screen.queryByRole("listbox", { name: /recipes/i })).toBeNull();

    // Type "/"
    act(() => {
      editorState.json = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "/" }],
          },
        ],
      };
      editorState.onUpdate?.(editorState.json);
    });

    // Slash menu should open and list built-in recipes
    const menu = screen.getByRole("listbox", { name: /recipes/i });
    expect(menu).toBeTruthy();
    expect(screen.getByText("Action items")).toBeTruthy();
    expect(screen.getByText("Key decisions")).toBeTruthy();
    expect(screen.getByText("Weekly digest")).toBeTruthy();
  });

  it("filters recipes as user types after '/'", () => {
    render(
      <ChatMessageInput
        draftKey="recipe-test-session-2"
        onSendMessage={vi.fn()}
      />,
    );

    // Type "/decis"
    act(() => {
      editorState.json = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "/decis" }],
          },
        ],
      };
      editorState.onUpdate?.(editorState.json);
    });

    // Should only show Key decisions
    expect(screen.getByText("Key decisions")).toBeTruthy();
    expect(screen.queryByText("Action items")).toBeNull();
  });

  it("selects a recipe on click: inserts prompt, sets recipe chip, and closes menu", () => {
    const onSendMessage = vi.fn();
    render(
      <ChatMessageInput
        draftKey="recipe-test-session-3"
        onSendMessage={onSendMessage}
      />,
    );

    // Type "/action"
    act(() => {
      editorState.json = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "/action" }],
          },
        ],
      };
      editorState.onUpdate?.(editorState.json);
    });

    // Click on "Action items"
    const actionItemButton = screen.getByText("Action items");
    fireEvent.click(actionItemButton);

    // Menu should close
    expect(screen.queryByRole("listbox", { name: /recipes/i })).toBeNull();

    // Active recipe chip should be visible
    const chip = screen.getByText("Action items");
    expect(chip).toBeTruthy();
    expect(document.querySelector("[data-recipe-chip]")).toBeTruthy();
  });

  it("navigates recipes with arrow keys and selects with Enter", () => {
    const onSendMessage = vi.fn();
    const { container } = render(
      <ChatMessageInput
        draftKey="recipe-test-session-4"
        onSendMessage={onSendMessage}
      />,
    );

    // Type "/"
    act(() => {
      editorState.json = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "/" }],
          },
        ],
      };
      editorState.onUpdate?.(editorState.json);
    });

    const inputWrapper = container.querySelector("[data-chat-message-input]")!;

    // Press ArrowDown to select next recipe
    fireEvent.keyDown(inputWrapper, { key: "ArrowDown" });

    // Press Enter to confirm recipe selection
    fireEvent.keyDown(inputWrapper, { key: "Enter" });

    // Message should NOT have been sent
    expect(onSendMessage).not.toHaveBeenCalled();

    // Menu closed and recipe chip is set
    expect(screen.queryByRole("listbox", { name: /recipes/i })).toBeNull();
    expect(document.querySelector("[data-recipe-chip]")).toBeTruthy();
  });

  it("sends message and clears recipe chip on Enter submission", () => {
    const onSendMessage = vi.fn();
    render(
      <ChatMessageInput
        draftKey="recipe-test-session-5"
        onSendMessage={onSendMessage}
      />,
    );

    // Type "/" and select Action items
    act(() => {
      editorState.json = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "/action" }],
          },
        ],
      };
      editorState.onUpdate?.(editorState.json);
    });

    fireEvent.click(screen.getByText("Action items"));
    expect(document.querySelector("[data-recipe-chip]")).toBeTruthy();

    // Editor now has the recipe prompt
    editorState.json = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Identify all action items..." }],
        },
      ],
    };

    // Press Enter (now that menu is closed)
    act(() => {
      editorState.onSubmit?.();
    });

    expect(onSendMessage).toHaveBeenCalledWith(
      "Identify all action items...",
      [{ type: "text", text: "Identify all action items..." }],
      [],
    );

    // Recipe chip should be cleared
    expect(document.querySelector("[data-recipe-chip]")).toBeNull();
  });

  it("allows removing the recipe chip via its remove button", () => {
    render(
      <ChatMessageInput
        draftKey="recipe-test-session-6"
        onSendMessage={vi.fn()}
      />,
    );

    // Type "/key" and select Key decisions
    act(() => {
      editorState.json = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "/key" }],
          },
        ],
      };
      editorState.onUpdate?.(editorState.json);
    });

    fireEvent.click(screen.getByText("Key decisions"));
    expect(document.querySelector("[data-recipe-chip]")).toBeTruthy();

    // Click remove button on chip
    const removeButton = screen.getByRole("button", {
      name: /remove recipe/i,
    });
    fireEvent.click(removeButton);

    // Recipe chip should be removed
    expect(document.querySelector("[data-recipe-chip]")).toBeNull();
  });
});
