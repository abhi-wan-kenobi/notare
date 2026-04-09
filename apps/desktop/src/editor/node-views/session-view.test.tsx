import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => {
  const transaction = {
    setNodeMarkup: vi.fn(),
  };
  const view = {
    state: { tr: transaction },
    dispatch: vi.fn(),
  };
  const openCurrent = vi.fn();
  const openNew = vi.fn();

  return { transaction, view, openCurrent, openNew };
});

vi.mock("@handlewithcare/react-prosemirror", () => ({
  useEditorEventCallback:
    (callback: (view: typeof hoisted.view) => void) => () =>
      callback(hoisted.view),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useRow: () => ({
      created_at: "2026-04-06T00:00:00.000Z",
      event_json: JSON.stringify({
        ended_at: "2026-04-06T01:00:00.000Z",
      }),
    }),
  },
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: { sessionId: string | null; status: string };
    }) => unknown,
  ) => selector({ live: { sessionId: null, status: "inactive" } }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      openCurrent: typeof hoisted.openCurrent;
      openNew: typeof hoisted.openNew;
    }) => unknown,
  ) =>
    selector({
      openCurrent: hoisted.openCurrent,
      openNew: hoisted.openNew,
    }),
}));

vi.mock("~/editor/session/linked-item-open-behavior", () => ({
  useLinkedItemOpenBehavior: () => "current",
}));

import { SessionNodeView } from "./session-view";

describe("SessionNodeView", () => {
  it("cycles the linked session status when clicked", () => {
    hoisted.transaction.setNodeMarkup.mockImplementation(
      (_pos, _type, attrs) => ({ attrs }),
    );
    hoisted.view.dispatch.mockClear();

    render(
      <SessionNodeView
        nodeProps={
          {
            node: {
              attrs: { sessionId: "session-1", status: "done", checked: true },
            },
            getPos: () => 7,
          } as any
        }
      >
        Meeting
      </SessionNodeView>,
    );

    fireEvent.click(screen.getByRole("checkbox"));

    expect(hoisted.transaction.setNodeMarkup).toHaveBeenCalledWith(
      7,
      undefined,
      {
        sessionId: "session-1",
        status: "todo",
        checked: false,
      },
    );
    expect(hoisted.view.dispatch).toHaveBeenCalledWith({
      attrs: {
        sessionId: "session-1",
        status: "todo",
        checked: false,
      },
    });
  });

  it("opens the linked session when clicking outside the editable title", () => {
    hoisted.openCurrent.mockClear();

    const { container } = render(
      <SessionNodeView
        nodeProps={
          {
            node: {
              attrs: { sessionId: "session-1", status: "todo", checked: false },
            },
            getPos: () => 7,
          } as any
        }
      >
        Meeting
      </SessionNodeView>,
    );

    const row = container.querySelector("[data-session-row]");

    expect(row).not.toBeNull();

    fireEvent.click(row!);

    expect(hoisted.openCurrent).toHaveBeenCalledWith({
      id: "session-1",
      type: "sessions",
    });
  });

  it("opens the linked session when clicking the title", () => {
    hoisted.openCurrent.mockClear();

    const { container } = render(
      <SessionNodeView
        nodeProps={
          {
            node: {
              attrs: { sessionId: "session-1", status: "todo", checked: false },
            },
            getPos: () => 7,
          } as any
        }
      >
        Meeting
      </SessionNodeView>,
    );

    const title = container.querySelector("[data-session-title]");

    expect(title).not.toBeNull();

    fireEvent.click(title!);

    expect(hoisted.openCurrent).toHaveBeenCalledWith({
      id: "session-1",
      type: "sessions",
    });
  });
});
