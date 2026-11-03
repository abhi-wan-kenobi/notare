import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const calendarCommands = vi.hoisted(() => ({
  listConnectionIds: vi.fn(),
  listCalendars: vi.fn(),
  createEvent: vi.fn(),
}));

const mocks = vi.hoisted(() => ({
  useSession: vi.fn(),
  useSessionParticipants: vi.fn(),
  getSessionEvent: vi.fn(),
  useSessionActionItems: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: calendarCommands,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (input: TemplateStringsArray | string, ...values: unknown[]) => {
      if (typeof input === "string") return input;
      return (input as readonly string[]).reduce(
        (message: string, part: string, index: number) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );
    },
  }),
}));

vi.mock("@hypr/ui/components/ui/dropdown-menu", () => ({
  DropdownMenuItem: ({
    onClick,
    children,
  }: {
    onClick?: () => void;
    children?: ReactNode;
  }) => (
    <button type="button" onClick={onClick}>
      {children}
    </button>
  ),
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { success: mocks.toastSuccess, error: mocks.toastError },
}));

vi.mock("~/session/queries", () => ({
  useSession: mocks.useSession,
  useSessionParticipants: mocks.useSessionParticipants,
}));

vi.mock("~/session/utils", () => ({
  getSessionEvent: mocks.getSessionEvent,
}));

vi.mock("~/session/components/action-items/queries", () => ({
  useSessionActionItems: mocks.useSessionActionItems,
}));

import {
  ScheduleFollowupDialog,
  ScheduleFollowupMenuItem,
} from "./schedule-followup";

function renderWithClient(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}

const SESSION_ID = "session-1";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.useSession.mockReturnValue({
    id: SESSION_ID,
    title: "Weekly sync",
    event_json: "",
  });
  mocks.getSessionEvent.mockReturnValue(null);
  mocks.useSessionParticipants.mockReturnValue([
    { humanId: "h1", name: "Alice", email: "alice@example.com" },
    { humanId: "h2", name: "No Email", email: "" },
  ]);
  mocks.useSessionActionItems.mockReturnValue({
    items: [{ text: "Send proposal", status: "todo" }],
    isLoading: false,
  });
});

afterEach(() => {
  cleanup();
});

describe("ScheduleFollowupMenuItem", () => {
  it("is hidden when there are no writable calendars", async () => {
    calendarCommands.listConnectionIds.mockResolvedValue({
      status: "ok",
      data: [],
    });

    renderWithClient(<ScheduleFollowupMenuItem onSelect={vi.fn()} />);

    await waitFor(() => {
      expect(calendarCommands.listConnectionIds).toHaveBeenCalled();
    });
    expect(screen.queryByText("Schedule follow-up")).toBeNull();
  });

  it("is shown and calls onSelect when writable calendars exist", async () => {
    calendarCommands.listConnectionIds.mockResolvedValue({
      status: "ok",
      data: [{ provider: "google", connection_ids: ["google:a@b.com"] }],
    });
    calendarCommands.listCalendars.mockResolvedValue({
      status: "ok",
      data: [{ id: "primary", title: "Primary", can_edit: true }],
    });
    const onSelect = vi.fn();

    renderWithClient(<ScheduleFollowupMenuItem onSelect={onSelect} />);

    const item = await screen.findByText("Schedule follow-up");
    fireEvent.click(item);
    expect(onSelect).toHaveBeenCalledOnce();
  });
});

describe("ScheduleFollowupDialog", () => {
  function mockOneWritableCalendar() {
    calendarCommands.listConnectionIds.mockResolvedValue({
      status: "ok",
      data: [{ provider: "google", connection_ids: ["google:a@example.com"] }],
    });
    calendarCommands.listCalendars.mockResolvedValue({
      status: "ok",
      data: [{ id: "primary", title: "Primary", can_edit: true }],
    });
  }

  it("prefills title, attendees (emails only), and description from action items", async () => {
    mockOneWritableCalendar();

    renderWithClient(
      <ScheduleFollowupDialog
        sessionId={SESSION_ID}
        open={true}
        onOpenChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByDisplayValue("Weekly sync — follow-up")).toBeDefined();
    });
    expect(screen.getByText("Alice")).toBeDefined();
    expect(screen.queryByText("No Email")).toBeNull();
    expect(screen.getByDisplayValue("- Send proposal")).toBeDefined();
  });

  it("removes an attendee when its remove button is clicked", async () => {
    mockOneWritableCalendar();

    renderWithClient(
      <ScheduleFollowupDialog
        sessionId={SESSION_ID}
        open={true}
        onOpenChange={vi.fn()}
      />,
    );

    await screen.findByText("Alice");
    fireEvent.click(screen.getByLabelText("Remove Alice"));
    expect(screen.queryByText("Alice")).toBeNull();
  });

  it("submits createEvent with the selected calendar, computed times, and attendees", async () => {
    mockOneWritableCalendar();
    calendarCommands.createEvent.mockResolvedValue({
      status: "ok",
      data: "event-123",
    });

    renderWithClient(
      <ScheduleFollowupDialog
        sessionId={SESSION_ID}
        open={true}
        onOpenChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByDisplayValue("Weekly sync — follow-up")).toBeDefined();
    });
    await waitFor(() => {
      expect(screen.getAllByRole("combobox")[1]!.textContent).toBe(
        "Primary — a@example.com",
      );
    });

    fireEvent.click(screen.getByText("Schedule"));

    await waitFor(() => {
      expect(calendarCommands.createEvent).toHaveBeenCalledOnce();
    });

    const [provider, connectionId, input] =
      calendarCommands.createEvent.mock.calls[0]!;
    expect(provider).toBe("google");
    expect(connectionId).toBe("google:a@example.com");
    expect(input.calendar_tracking_id).toBe("primary");
    expect(input.title).toBe("Weekly sync — follow-up");
    expect(input.attendees).toEqual([
      { email: "alice@example.com", name: "Alice" },
    ]);
    // Default duration is 30 minutes.
    expect(
      new Date(input.ended_at).getTime() - new Date(input.started_at).getTime(),
    ).toBe(30 * 60_000);

    await waitFor(() => {
      expect(mocks.toastSuccess).toHaveBeenCalledOnce();
    });
  });

  it("surfaces a backend error via toast and inline message instead of throwing", async () => {
    mockOneWritableCalendar();
    calendarCommands.createEvent.mockResolvedValue({
      status: "error",
      error: "consent denied",
    });

    renderWithClient(
      <ScheduleFollowupDialog
        sessionId={SESSION_ID}
        open={true}
        onOpenChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByDisplayValue("Weekly sync — follow-up")).toBeDefined();
    });
    await waitFor(() => {
      expect(screen.getAllByRole("combobox")[1]!.textContent).toBe(
        "Primary — a@example.com",
      );
    });

    fireEvent.click(screen.getByText("Schedule"));

    await waitFor(() => {
      expect(mocks.toastError).toHaveBeenCalledWith("consent denied");
    });
    expect(screen.getByText("consent denied")).toBeDefined();
  });
});
