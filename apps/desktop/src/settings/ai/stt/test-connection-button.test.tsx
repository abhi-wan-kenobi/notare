import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { TestConnectionButton } from "./test-connection-button";

const { fetchMock } = vi.hoisted(() => ({ fetchMock: vi.fn() }));

vi.mock("@tauri-apps/plugin-http", () => ({
  fetch: fetchMock,
}));

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
});

describe("TestConnectionButton", () => {
  test("is disabled until a base URL is entered", () => {
    render(<TestConnectionButton baseUrl="" apiKey="" />);

    const button = screen.getByRole("button", {
      name: /test connection/i,
    }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  test("shows engine, GPU offload, and the loaded model on success", async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => ({
        engine: "whisper-local",
        gpuOffload: "verified",
        loadedModel: { id: "QuantizedLargeTurbo", file: "ggml-large-v3-turbo-q8_0.bin" },
        version: "0.1.0",
      }),
    });

    render(
      <TestConnectionButton baseUrl="http://192.168.0.91:8383/v1" apiKey="" />,
    );

    fireEvent.click(screen.getByRole("button", { name: /test connection/i }));

    await waitFor(() => {
      expect(screen.getByText(/whisper-local/)).toBeTruthy();
    });

    expect(screen.getByText(/verified/)).toBeTruthy();
    expect(screen.getByText(/QuantizedLargeTurbo/)).toBeTruthy();
    expect(fetchMock).toHaveBeenCalledWith(
      "http://192.168.0.91:8383/api/status",
      expect.objectContaining({ method: "GET" }),
    );
  });

  test("shows a clear failure message when the server is unreachable", async () => {
    fetchMock.mockRejectedValueOnce(new Error("connection refused"));

    render(
      <TestConnectionButton baseUrl="http://192.168.0.91:9999/v1" apiKey="" />,
    );

    fireEvent.click(screen.getByRole("button", { name: /test connection/i }));

    await waitFor(() => {
      expect(screen.getByText(/connection refused/)).toBeTruthy();
    });
  });

  test("shows a failure message for a non-2xx response", async () => {
    fetchMock.mockResolvedValueOnce({ ok: false, status: 401 });

    render(
      <TestConnectionButton baseUrl="http://192.168.0.91:8383/v1" apiKey="wrong" />,
    );

    fireEvent.click(screen.getByRole("button", { name: /test connection/i }));

    await waitFor(() => {
      expect(screen.getByText(/401/)).toBeTruthy();
    });
  });
});
