import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LocalModelManager } from "./select";

type FakeChannel = { onmessage: (progress: number) => void };

const mocks = vi.hoisted(() => ({
  ask: vi.fn(),
  toastError: vi.fn(),
  listSupportedModel: vi.fn(),
  listDownloadedModel: vi.fn(),
  downloadModel: vi.fn(),
  cancelDownload: vi.fn(),
  deleteModel: vi.fn(),
  channels: [] as FakeChannel[],
}));

vi.mock("@tauri-apps/api/core", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tauri-apps/api/core")>()),
  Channel: class {
    onmessage: (progress: number) => void = () => {};
    constructor() {
      mocks.channels.push(this);
    }
  },
}));

vi.mock("@tauri-apps/plugin-dialog", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tauri-apps/plugin-dialog")>()),
  ask: mocks.ask,
}));

vi.mock("@hypr/ui/components/ui/toast", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@hypr/ui/components/ui/toast")>()),
  sonnerToast: { error: mocks.toastError },
}));

vi.mock("@hypr/plugin-local-llm", () => ({
  commands: {
    listSupportedModel: mocks.listSupportedModel,
    listDownloadedModel: mocks.listDownloadedModel,
    downloadModel: mocks.downloadModel,
    cancelDownload: mocks.cancelDownload,
    deleteModel: mocks.deleteModel,
  },
}));

vi.mock("~/shared/config", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/shared/config")>()),
  useConfigValue: () => true,
}));

vi.mock("~/settings/queries", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/settings/queries")>()),
  useSetSettingValue: () => vi.fn(),
}));

const MODEL = {
  key: "Gemma3_4bQ4",
  name: "Gemma 3 4B",
  description: "Small general model",
  size_bytes: 2_500_000_000,
};

function renderManager(downloaded: string[]) {
  mocks.listSupportedModel.mockResolvedValue({ status: "ok", data: [MODEL] });
  mocks.listDownloadedModel.mockResolvedValue({
    status: "ok",
    data: downloaded,
  });
  const onSelectModel = vi.fn();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <LocalModelManager
        selectedModelId={MODEL.key}
        onSelectModel={onSelectModel}
        onModelDownloaded={vi.fn()}
      />
    </QueryClientProvider>,
  );
  return { onSelectModel };
}

describe("LocalModelManager", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.restoreAllMocks();
    mocks.channels.length = 0;
  });

  describe("delete", () => {
    it("keeps the model when the confirmation is declined", async () => {
      mocks.ask.mockResolvedValue(false);
      const { onSelectModel } = renderManager([MODEL.key]);

      fireEvent.click(await screen.findByRole("button", { name: "Delete" }));

      await waitFor(() => expect(mocks.ask).toHaveBeenCalled());
      expect(mocks.ask.mock.calls[0][0]).toContain(MODEL.name);
      expect(mocks.deleteModel).not.toHaveBeenCalled();
      expect(onSelectModel).not.toHaveBeenCalled();
    });

    it("deletes and clears the selection once confirmed", async () => {
      mocks.ask.mockResolvedValue(true);
      mocks.deleteModel.mockResolvedValue({ status: "ok", data: null });
      const { onSelectModel } = renderManager([MODEL.key]);

      fireEvent.click(await screen.findByRole("button", { name: "Delete" }));

      await waitFor(() => expect(onSelectModel).toHaveBeenCalledWith(""));
      expect(mocks.deleteModel).toHaveBeenCalledWith(MODEL.key);
      expect(mocks.toastError).not.toHaveBeenCalled();
    });

    it("reports a failed delete and keeps the selection", async () => {
      mocks.ask.mockResolvedValue(true);
      mocks.deleteModel.mockResolvedValue({
        status: "error",
        error: "file is in use",
      });
      const { onSelectModel } = renderManager([MODEL.key]);

      fireEvent.click(await screen.findByRole("button", { name: "Delete" }));

      await waitFor(() => expect(mocks.toastError).toHaveBeenCalledTimes(1));
      expect(onSelectModel).not.toHaveBeenCalled();
    });
  });

  describe("download", () => {
    it("reports a download that fails to start", async () => {
      mocks.downloadModel.mockResolvedValue({
        status: "error",
        error: "no download url",
      });
      renderManager([]);

      fireEvent.click(await screen.findByRole("button", { name: "Download" }));

      await waitFor(() => expect(mocks.toastError).toHaveBeenCalledTimes(1));
      expect(
        await screen.findByRole("button", { name: "Download" }),
      ).toBeTruthy();
    });

    it("reports a download that fails after it started", async () => {
      mocks.downloadModel.mockResolvedValue({ status: "ok", data: null });
      renderManager([]);

      fireEvent.click(await screen.findByRole("button", { name: "Download" }));
      await waitFor(() => expect(mocks.downloadModel).toHaveBeenCalled());
      act(() => mocks.channels[0].onmessage(40));
      expect(screen.getByText("40%")).toBeTruthy();

      act(() => mocks.channels[0].onmessage(-1));

      expect(mocks.toastError).toHaveBeenCalledTimes(1);
      expect(screen.getByRole("button", { name: "Download" })).toBeTruthy();
    });

    it("does not report a download the user cancelled", async () => {
      mocks.downloadModel.mockResolvedValue({ status: "ok", data: null });
      mocks.cancelDownload.mockImplementation(async () => {
        mocks.channels[0].onmessage(-1);
        return { status: "ok", data: true };
      });
      renderManager([]);

      fireEvent.click(await screen.findByRole("button", { name: "Download" }));
      await waitFor(() => expect(mocks.downloadModel).toHaveBeenCalled());
      act(() => mocks.channels[0].onmessage(40));

      fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

      expect(
        await screen.findByRole("button", { name: "Download" }),
      ).toBeTruthy();
      expect(mocks.toastError).not.toHaveBeenCalled();
    });
  });
});
