import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Listening } from "./listening";

const {
  isMainWebviewWindowMock,
  requestMainListenerControlMock,
  startListeningMock,
  stopMock,
  useListenerMock,
} = vi.hoisted(() => ({
  isMainWebviewWindowMock: vi.fn(() => true),
  requestMainListenerControlMock: vi.fn(),
  startListeningMock: vi.fn(),
  stopMock: vi.fn(),
  useListenerMock: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/dropdown-menu", () => ({
  DropdownMenuItem: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>
      {children}
    </button>
  ),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("~/stt/useStartListening", () => ({
  useStartListening: () => startListeningMock,
}));

vi.mock("~/stt/window-control", () => ({
  isMainWebviewWindow: isMainWebviewWindowMock,
  requestMainListenerControl: requestMainListenerControlMock,
}));

describe("Listening", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isMainWebviewWindowMock.mockReturnValue(true);
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "inactive",
        stop: stopMock,
      }),
    );
  });

  afterEach(() => {
    cleanup();
  });

  it("hides resume listening when a transcript already exists", () => {
    render(<Listening sessionId="session-1" hasTranscript />);

    expect(
      screen.queryByRole("button", { name: "Resume listening" }),
    ).toBeNull();
  });

  it("starts listening directly in the main window before transcript exists", () => {
    render(<Listening sessionId="session-1" hasTranscript={false} />);

    fireEvent.click(screen.getByRole("button", { name: "Start listening" }));

    expect(startListeningMock).toHaveBeenCalledTimes(1);
    expect(requestMainListenerControlMock).not.toHaveBeenCalled();
  });

  it("delegates standalone start requests to the main window before transcript exists", () => {
    isMainWebviewWindowMock.mockReturnValue(false);

    render(<Listening sessionId="session-1" hasTranscript={false} />);

    fireEvent.click(screen.getByRole("button", { name: "Start listening" }));

    expect(requestMainListenerControlMock).toHaveBeenCalledWith(
      "start",
      "session-1",
    );
    expect(startListeningMock).not.toHaveBeenCalled();
  });

  it("delegates standalone stop requests to the main window", () => {
    isMainWebviewWindowMock.mockReturnValue(false);
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "active",
        stop: stopMock,
      }),
    );

    render(<Listening sessionId="session-1" hasTranscript />);

    fireEvent.click(screen.getByRole("button", { name: "Stop listening" }));

    expect(requestMainListenerControlMock).toHaveBeenCalledWith(
      "stop",
      "session-1",
    );
    expect(stopMock).not.toHaveBeenCalled();
  });
});
