import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  live: {
    status: "inactive" as "inactive" | "active" | "finalizing",
    sessionId: null as string | null,
    seconds: 0,
  },
  sessionMode: "inactive" as "inactive" | "running_batch",
  connection: {
    isLocalModel: true,
    local: { data: { status: "ready" } },
    conn: {
      provider: "hyprnote",
      model: "parakeet-tdt-v3-int8",
      baseUrl: "http://localhost:8080",
      apiKey: "",
    },
  },
  start: vi.fn(),
  stop: vi.fn(),
  createSession: vi.fn(),
  openNew: vi.fn(),
  openCurrent: vi.fn(),
  setExpanded: vi.fn(),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({
      live: mocks.live,
      start: mocks.start,
      stop: mocks.stop,
      getSessionMode: () => mocks.sessionMode,
    }),
}));

vi.mock("~/stt/useSTTConnection", () => ({
  useSTTConnection: () => mocks.connection,
}));

vi.mock("~/session/queries", () => ({
  createSession: mocks.createSession,
  useSession: () => ({ id: "session-1", user_id: "user-1", raw_md: "" }),
  useSessionHasTranscript: () => false,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({ openNew: mocks.openNew, openCurrent: mocks.openCurrent }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: { setExpanded: mocks.setExpanded },
    inspector: { openWith: vi.fn() },
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => {
    if (key === "live_transcription_enabled") return true;
    if (key === "ai_language") return "en";
    return undefined;
  },
}));

vi.mock("~/stt/useKeywords", () => ({
  getSessionKeywords: vi.fn(async () => []),
}));

vi.mock("~/stt/useRunBatch", () => ({
  canRunBatchTranscription: () => true,
  isStoppedTranscriptionError: () => false,
  useRunBatch: () => vi.fn(),
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => null,
}));

vi.mock("~/services/audio-retention", () => ({
  normalizeAudioRetention: () => "forever",
  deleteProcessedAudioForRetention: vi.fn(),
}));

vi.mock("~/session/utils", () => ({ getSessionEvent: () => null }));
vi.mock("~/shared/utils", () => ({ id: () => "transcript-1" }));
vi.mock("~/stt/queries", () => ({
  useSessionParticipantHumanIds: () => [],
  createLiveTranscript: vi.fn(),
  applyLiveTranscriptDeltaToDatabase: vi.fn(),
  softDeleteTranscript: vi.fn(),
}));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: {
    isSupportedLanguagesLive: vi.fn(async () => ({ status: "ok", data: true })),
  },
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: { event: vi.fn() },
}));

import { Dock } from "./index";

describe("Dock recording start", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.live.status = "inactive";
    mocks.live.sessionId = null;
    mocks.sessionMode = "inactive";
    mocks.connection.isLocalModel = true;
    mocks.connection.local.data.status = "ready";
    mocks.start.mockResolvedValue(true);
    mocks.createSession.mockResolvedValue("new-session");
  });

  afterEach(cleanup);

  it("starts capture for the selected session using the configured local model", async () => {
    render(<Dock sessionId="session-1" />);
    fireEvent.click(screen.getByRole("button", { name: "Start recording" }));

    await waitFor(() => expect(mocks.start).toHaveBeenCalledOnce());
    expect(mocks.start).toHaveBeenCalledWith(
      expect.objectContaining({
        session_id: "session-1",
        model: "parakeet-tdt-v3-int8",
        base_url: "http://localhost:8080",
        transcription_mode: "live",
      }),
      expect.objectContaining({ onStopped: expect.any(Function) }),
    );
    expect(mocks.createSession).not.toHaveBeenCalled();
  });

  it("creates a home note with auto-start enabled", async () => {
    render(<Dock />);
    fireEvent.click(screen.getByRole("button", { name: "Start recording" }));

    await waitFor(() =>
      expect(mocks.openNew).toHaveBeenCalledWith({
        type: "sessions",
        id: "new-session",
        state: { view: null, autoStart: true },
      }),
    );
    expect(mocks.createSession).toHaveBeenCalledOnce();
  });

  it.each([undefined, "session-1"])(
    "does not create a note or start capture without a ready local model (session: %s)",
    (sessionId) => {
      mocks.connection.isLocalModel = false;
      render(<Dock sessionId={sessionId} />);
      fireEvent.click(screen.getByRole("button", { name: "Start recording" }));

      expect(mocks.createSession).not.toHaveBeenCalled();
      expect(mocks.start).not.toHaveBeenCalled();
      expect(mocks.openNew).not.toHaveBeenCalled();
    },
  );

  it.each([undefined, "session-1"])(
    "does not start capture while the local model is still loading (session: %s)",
    (sessionId) => {
      mocks.connection.local.data.status = "loading";
      render(<Dock sessionId={sessionId} />);
      fireEvent.click(screen.getByRole("button", { name: "Start recording" }));

      expect(mocks.createSession).not.toHaveBeenCalled();
      expect(mocks.start).not.toHaveBeenCalled();
    },
  );

  it("does not reach capture start during the selected session's batch run", () => {
    mocks.sessionMode = "running_batch";
    render(<Dock sessionId="session-1" />);
    fireEvent.click(screen.getByRole("button", { name: "Start recording" }));

    expect(mocks.start).not.toHaveBeenCalled();
    expect(mocks.createSession).not.toHaveBeenCalled();
  });
});
