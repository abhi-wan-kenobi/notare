import { beforeEach, describe, expect, test } from "vitest";

import { useTabs } from ".";
import { createSessionTab, resetTabsStore } from "./test-utils";

describe("Chat Mode", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("initial mode is FloatingClosed", () => {
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("TOGGLE from FloatingClosed → FloatingOpen", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });

  test("TOGGLE from FloatingOpen → FloatingClosed", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("SHIFT from FloatingOpen → RightPanelOpen", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().transitionChatMode({ type: "SHIFT" });
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");
  });

  test("OPEN_FLOATING from FloatingClosed → FloatingOpen", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN_FLOATING" });
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });

  test("OPEN_RIGHT_PANEL from FloatingClosed → RightPanelOpen", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN_RIGHT_PANEL" });
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");
  });

  test("no-op when event is irrelevant for current state", () => {
    useTabs.getState().transitionChatMode({ type: "CLOSE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("closing non-chat tab does not affect mode", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");

    const sessionTab = useTabs
      .getState()
      .tabs.find((t) => t.type === "sessions")!;
    useTabs.getState().close(sessionTab);
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });

  test("closeAll leaves the floating chat mode unchanged", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });

    useTabs.getState().closeAll();
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });
});
