import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ScopeSelector } from "./scope-selector";

import { useChatScopeStore } from "~/chat/state/chat-scope";

vi.mock("~/session/queries", () => ({
  useSessionSummaries: vi
    .fn()
    .mockReturnValue([
      { id: "session-1", title: "Team Sync", created_at: "2026-11-01" },
    ]),
}));

vi.mock("~/contacts/queries", () => ({
  useHumans: vi
    .fn()
    .mockReturnValue([
      { id: "human-1", name: "Alice", email: "alice@example.com" },
    ]),
  useOrganizations: vi
    .fn()
    .mockReturnValue([{ id: "org-1", name: "Acme Corp" }]),
}));

describe("ScopeSelector component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useChatScopeStore.setState({ scopesByGroupId: {} });
  });

  afterEach(() => {
    cleanup();
  });

  it("defaults to 'This meeting' when a session is open", () => {
    render(
      <ScopeSelector
        currentChatGroupId="group-1"
        currentSessionId="session-1"
      />,
    );

    const button = screen.getByRole("button", { name: "Chat scope" });
    expect(button).toBeDefined();
    expect(button.textContent).toContain("This meeting");
  });

  it("defaults to 'Everything' when no session is open", () => {
    render(
      <ScopeSelector
        currentChatGroupId="group-2"
        currentSessionId={undefined}
      />,
    );

    const button = screen.getByRole("button", { name: "Chat scope" });
    expect(button.textContent).toContain("Everything");
  });

  it("switches scope and persists per chat group in store", () => {
    render(
      <ScopeSelector
        currentChatGroupId="group-1"
        currentSessionId="session-1"
      />,
    );

    const trigger = screen.getByRole("button", { name: "Chat scope" });
    fireEvent.click(trigger);

    const everythingOption = screen.getByRole("button", {
      name: /Everything/i,
    });
    fireEvent.click(everythingOption);

    const saved = useChatScopeStore.getState().scopesByGroupId["group-1"];
    expect(saved).toEqual({ type: "everything" });
  });

  it("switches to 'Person' scope and associates first available human", () => {
    render(
      <ScopeSelector
        currentChatGroupId="group-1"
        currentSessionId="session-1"
      />,
    );

    const trigger = screen.getByRole("button", { name: "Chat scope" });
    fireEvent.click(trigger);

    const personOption = screen.getByRole("button", { name: /Person/i });
    fireEvent.click(personOption);

    const saved = useChatScopeStore.getState().scopesByGroupId["group-1"];
    expect(saved).toEqual({ type: "person", humanId: "human-1" });
  });

  it("switches to 'Organization' scope and associates first available org", () => {
    render(
      <ScopeSelector
        currentChatGroupId="group-1"
        currentSessionId="session-1"
      />,
    );

    const trigger = screen.getByRole("button", { name: "Chat scope" });
    fireEvent.click(trigger);

    const orgOption = screen.getByRole("button", { name: /Organization/i });
    fireEvent.click(orgOption);

    const saved = useChatScopeStore.getState().scopesByGroupId["group-1"];
    expect(saved).toEqual({ type: "organization", orgId: "org-1" });
  });
});
