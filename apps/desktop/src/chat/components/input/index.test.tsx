import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
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
    inspectorOpen: true,
  },
}));

vi.mock("@hypr/editor/chat", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

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
    inspector: {
      open: shellState.inspectorOpen,
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

describe("ChatMessageInput", () => {
  beforeEach(() => {
    cleanup();
    clearContentMock.mockClear();
    editorState.json = { type: "doc", content: [] };
    editorState.onSubmit = undefined;
    editorState.onUpdate = undefined;
    editorState.submitShortcut = undefined;
    shellState.inspectorOpen = true;
  });

  it("disables send until the draft has content", () => {
    const onSendMessage = vi.fn();
    const onDraftContentChange = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        onDraftContentChange={onDraftContentChange}
        onSendMessage={onSendMessage}
      />,
    );

    const sendButton = screen.getByRole("button", {
      name: /send/i,
    }) as HTMLButtonElement;
    expect(sendButton.disabled).toBe(true);

    editorState.json = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Hello" }],
        },
      ],
    };
    act(() => {
      editorState.onUpdate?.(editorState.json);
    });

    expect(sendButton.disabled).toBe(false);
    expect(onDraftContentChange).toHaveBeenCalledWith(true);

    fireEvent.click(sendButton);

    expect(onSendMessage).toHaveBeenCalledWith(
      "Hello",
      [{ type: "text", text: "Hello" }],
      [],
    );
    expect(clearContentMock).toHaveBeenCalled();
    expect(onDraftContentChange).toHaveBeenLastCalledWith(false);
  });

  it("tracks attachment-only drafts without enabling text send", () => {
    const onDraftContentChange = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        onDraftContentChange={onDraftContentChange}
        onSendMessage={vi.fn()}
      />,
    );

    const sendButton = screen.getByRole<HTMLButtonElement>("button", {
      name: /send/i,
    });

    editorState.json = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "attachment",
              attrs: {
                id: "attachment-1",
                name: "image.png",
                mimeType: "image/png",
                url: "data:image/png;base64,abc",
                size: 123,
              },
            },
          ],
        },
      ],
    };
    act(() => {
      editorState.onUpdate?.(editorState.json);
    });

    expect(onDraftContentChange).toHaveBeenCalledWith(true);
    expect(sendButton.disabled).toBe(true);
  });

  it("submits drafts while streaming so the caller can queue them", () => {
    const onSendMessage = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        isStreaming
        onSendMessage={onSendMessage}
      />,
    );

    editorState.json = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
      ],
    };
    act(() => {
      editorState.onUpdate?.(editorState.json);
      editorState.onSubmit?.();
    });

    expect(onSendMessage).toHaveBeenCalledWith(
      "Follow up",
      [{ type: "text", text: "Follow up" }],
      [],
    );
    expect(clearContentMock).toHaveBeenCalled();
  });

  it("configures Enter as the chat submit shortcut", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    expect(editorState.submitShortcut).toBe("enter");
  });

  it("marks the send control for disabled surface styling before the draft has content", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const sendButton = screen.getByRole<HTMLButtonElement>("button", {
      name: /send/i,
    });

    expect(sendButton.disabled).toBe(true);
    expect(sendButton.className).toContain("chat-input-send");
    expect(sendButton.className).not.toContain("bg-primary");
    expect(sendButton.className).toContain("rounded-full");
    expect(sendButton.className).toContain("size-7");
    expect(sendButton.textContent).toBe("");
    expect(sendButton.querySelector("svg")).not.toBeNull();
  });

  it("uses the elevated card input surface", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const editor = screen.getByTestId("chat-editor");
    const surface = editor.closest("[data-chat-message-input]")?.parentElement;

    expect(editor.className).toContain("chat-input-editor");
    expect(editor.className).toContain("max-h-[40vh]");
    expect(surface?.getAttribute("data-chat-input-surface")).toBe("elevated");
    expect(surface?.className).toContain("bg-card");
    expect(surface?.className).toContain("text-card-foreground");
    expect(surface?.className).toContain("rounded-xl");
  });

  it("uses balanced outer padding", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const messageInput = screen
      .getByTestId("chat-editor")
      .closest("[data-chat-message-input]");
    const outerContainer = messageInput?.parentElement?.parentElement;

    expect(outerContainer?.className).toContain("px-3");
    expect(outerContainer?.className).toContain("pb-4");
    expect(outerContainer?.className).not.toContain("px-5");
    expect(outerContainer?.className).not.toContain("px-2");
    expect(outerContainer?.className).not.toContain("pr-0");
  });

  it("caps the editor height at 40vh", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const editor = screen.getByTestId("chat-editor");

    expect(editor.className).toContain("max-h-[40vh]");
    expect(editor.className).not.toContain("max-h-48");
  });
});
