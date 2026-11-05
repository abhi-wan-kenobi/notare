import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  live: {
    status: "inactive" as "inactive" | "active" | "finalizing",
    seconds: 0,
    sessionId: null as string | null,
  },
  stop: vi.fn(),
  sttConnection: { isLocalModel: true, local: { data: { status: "ready" } } },
  startListeningThisSession: vi.fn(),
  startNewNoteAndListen: vi.fn(),
  inspectorOpenWith: vi.fn(),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: typeof mocks.live;
      stop: typeof mocks.stop;
    }) => unknown,
  ) => selector({ live: mocks.live, stop: mocks.stop }),
}));

vi.mock("~/stt/useSTTConnection", () => ({
  useSTTConnection: () => mocks.sttConnection,
}));

vi.mock("~/stt/useStartListening", () => ({
  useStartListening: () => mocks.startListeningThisSession,
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNoteAndListen: () => mocks.startNewNoteAndListen,
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    inspector: { openWith: mocks.inspectorOpenWith },
  }),
}));

import { Dock } from "./index";

describe("Dock", () => {
  beforeEach(() => {
    mocks.live.status = "inactive";
    mocks.live.seconds = 0;
    mocks.live.sessionId = null;
    mocks.stop.mockClear();
    mocks.sttConnection.isLocalModel = true;
    mocks.sttConnection.local = { data: { status: "ready" } };
    mocks.startListeningThisSession.mockClear();
    mocks.startNewNoteAndListen.mockClear();
    mocks.inspectorOpenWith.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows Ready and starts a new note + listen for the home dock", () => {
    render(<Dock />);

    expect(screen.getByText("Ready")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Start recording" }));

    expect(mocks.startNewNoteAndListen).toHaveBeenCalled();
    expect(mocks.startListeningThisSession).not.toHaveBeenCalled();
  });

  it("starts listening on the given session for a session dock", () => {
    render(<Dock sessionId="session-1" />);

    fireEvent.click(screen.getByRole("button", { name: "Start recording" }));

    expect(mocks.startListeningThisSession).toHaveBeenCalled();
    expect(mocks.startNewNoteAndListen).not.toHaveBeenCalled();
  });

  it("shows the elapsed time and stops recording for the active session", () => {
    mocks.live.status = "active";
    mocks.live.seconds = 125;
    mocks.live.sessionId = "session-1";

    render(<Dock sessionId="session-1" />);

    expect(screen.getByText("02:05")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Stop recording" }));

    expect(mocks.stop).toHaveBeenCalled();
  });

  it("does not show another session's recording as its own", () => {
    mocks.live.status = "active";
    mocks.live.sessionId = "other-session";

    render(<Dock sessionId="session-1" />);

    expect(screen.getByText("Ready")).toBeTruthy();
  });

  it("disables the record button when no local model is ready", () => {
    mocks.sttConnection.isLocalModel = false;

    render(<Dock />);

    expect(
      screen.getByRole<HTMLButtonElement>("button", {
        name: "Start recording",
      }).disabled,
    ).toBe(true);
  });

  it("opens the inspector with the typed prompt and clears the input", () => {
    render(<Dock />);

    const input = screen.getByPlaceholderText(
      "Ask anything across your meetings…",
    ) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Summarize this" } });
    fireEvent.submit(input);

    expect(mocks.inspectorOpenWith).toHaveBeenCalledWith("Summarize this");
    expect(input.value).toBe("");
  });

  it("does not open the inspector for an empty prompt", () => {
    render(<Dock />);

    const input = screen.getByPlaceholderText(
      "Ask anything across your meetings…",
    );
    fireEvent.submit(input);

    expect(mocks.inspectorOpenWith).not.toHaveBeenCalled();
  });
});
