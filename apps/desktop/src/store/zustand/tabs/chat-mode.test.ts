import { beforeEach, describe, expect, test } from "vitest";

import { useTabs } from ".";
import { createSessionTab, resetTabsStore } from "./test-utils";

describe("Chat Mode (inspector open/closed)", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("initial mode is Closed", () => {
    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("TOGGLE from Closed to Open", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("Open");
  });

  test("TOGGLE from Open to Closed", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("OPEN from Closed to Open", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("Open");
  });

  test("CLOSE from Open to Closed", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().transitionChatMode({ type: "CLOSE" });
    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("no-op when already in the target state", () => {
    useTabs.getState().transitionChatMode({ type: "CLOSE" });
    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("closing non-chat tab does not affect mode", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("Open");

    const sessionTab = useTabs
      .getState()
      .tabs.find((t) => t.type === "sessions")!;
    useTabs.getState().close(sessionTab);
    expect(useTabs.getState().chatMode).toBe("Open");
  });

  test("opening a different session closes the inspector", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().openNew(second);

    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("opening the current session keeps the inspector open", () => {
    const session = createSessionTab({ id: "session" });

    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().openNew(createSessionTab({ id: session.id }));

    expect(useTabs.getState().chatMode).toBe("Open");
  });

  test("selecting a different session closes the inspector", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);
    const firstTab = useTabs
      .getState()
      .tabs.find((tab) => tab.type === "sessions" && tab.id === first.id)!;

    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().select(firstTab);

    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("cycling to the next session closes the inspector", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);
    useTabs
      .getState()
      .select(
        useTabs
          .getState()
          .tabs.find((tab) => tab.type === "sessions" && tab.id === first.id)!,
      );

    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().selectNext();

    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("cycling to the previous session closes the inspector", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);

    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().selectPrev();

    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("closing the active session closes the inspector when another session becomes active", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);
    const secondTab = useTabs
      .getState()
      .tabs.find((tab) => tab.type === "sessions" && tab.id === second.id)!;

    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().close(secondTab);

    expect(useTabs.getState().chatMode).toBe("Closed");
  });

  test("closeAll leaves the inspector mode unchanged", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });

    useTabs.getState().closeAll();
    expect(useTabs.getState().chatMode).toBe("Open");
  });
});
