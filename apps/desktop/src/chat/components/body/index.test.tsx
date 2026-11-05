import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./empty", () => ({
  ChatBodyEmpty: () => <div data-testid="chat-body-empty" />,
}));

vi.mock("./non-empty", () => ({
  ChatBodyNonEmpty: () => <div data-testid="chat-body-non-empty" />,
}));

vi.mock("./use-chat-auto-scroll", () => ({
  useChatAutoScroll: () => ({
    contentRef: { current: null },
    handleWheel: vi.fn(),
    isAtBottom: true,
    scrollRef: { current: null },
    scrollToBottom: vi.fn(),
    showGoToRecent: false,
    updateAutoScrollState: vi.fn(),
  }),
}));

import { ChatBody } from "./index";

describe("ChatBody", () => {
  beforeEach(() => {
    cleanup();
  });

  it("uses balanced content padding filling the docked drawer", () => {
    render(<ChatBody messages={[]} status="ready" />);

    const content = screen.getByTestId("chat-body-empty").parentElement;
    const scrollArea = content?.parentElement;
    const root = scrollArea?.parentElement;

    expect(content?.className).toContain("px-3");
    expect(content?.className).toContain("py-5");
    expect(content?.className).toContain("min-h-full");
    expect(scrollArea?.className).toContain("flex-1");
    expect(root?.className).toContain("flex-1");
    expect(content?.className).not.toContain("px-5");
    expect(content?.className).not.toContain("px-2");
  });
});
