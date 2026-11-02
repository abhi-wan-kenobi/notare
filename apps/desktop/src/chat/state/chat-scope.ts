import { create } from "zustand";

import type { ContextRef } from "~/chat/context/entities";

export type ChatScopeType =
  | "this-meeting"
  | "selected"
  | "person"
  | "organization"
  | "everything";

export type ChatScopeSelection = {
  type: ChatScopeType;
  humanId?: string;
  orgId?: string;
  sessionIds?: string[];
};

export type ToolScopeFilter =
  | { kind: "session_ids"; sessionIds: string[] }
  | { kind: "human_id"; humanId: string }
  | { kind: "org_id"; orgId: string }
  | { kind: "none" };

export interface ResolvedScope {
  filter: ToolScopeFilter;
  isThisMeeting: boolean;
}

export const NEW_CHAT_SCOPE_KEY = "__new_chat__";
export const DEFAULT_THIS_MEETING_SCOPE: ChatScopeSelection = {
  type: "this-meeting",
};
export const DEFAULT_EVERYTHING_SCOPE: ChatScopeSelection = {
  type: "everything",
};

interface ChatScopeState {
  scopesByGroupId: Record<string, ChatScopeSelection>;
  setScopeForGroup: (
    groupId: string | undefined,
    scope: ChatScopeSelection,
  ) => void;
  getScopeForGroup: (
    groupId: string | undefined,
    currentSessionId?: string,
  ) => ChatScopeSelection;
  resetGroupScope: (groupId: string | undefined) => void;
}

export const useChatScopeStore = create<ChatScopeState>((set, get) => ({
  scopesByGroupId: {},

  setScopeForGroup: (groupId, scope) => {
    const key = groupId ?? NEW_CHAT_SCOPE_KEY;
    set((state) => ({
      scopesByGroupId: {
        ...state.scopesByGroupId,
        [key]: scope,
      },
    }));
  },

  getScopeForGroup: (groupId, currentSessionId) => {
    const state = get();
    const key = groupId ?? NEW_CHAT_SCOPE_KEY;
    const existing = state.scopesByGroupId[key];
    if (existing) {
      return existing;
    }

    if (currentSessionId) {
      return DEFAULT_THIS_MEETING_SCOPE;
    }

    return DEFAULT_EVERYTHING_SCOPE;
  },

  resetGroupScope: (groupId) => {
    const key = groupId ?? NEW_CHAT_SCOPE_KEY;
    set((state) => {
      const next = { ...state.scopesByGroupId };
      delete next[key];
      return { scopesByGroupId: next };
    });
  },
}));

export function resolveScopeFilter(
  scope: ChatScopeSelection,
  currentSessionId?: string,
  contextRefs?: ContextRef[],
): ResolvedScope {
  if (scope.type === "this-meeting") {
    if (currentSessionId) {
      return {
        filter: { kind: "session_ids", sessionIds: [currentSessionId] },
        isThisMeeting: true,
      };
    }
    return {
      filter: { kind: "none" },
      isThisMeeting: true,
    };
  }

  if (scope.type === "selected") {
    const referencedSessionIds = (contextRefs ?? [])
      .filter((ref) => ref.kind === "session")
      .map((ref) => ref.sessionId);

    const sessionIds = scope.sessionIds ?? referencedSessionIds;
    return {
      filter: { kind: "session_ids", sessionIds },
      isThisMeeting: false,
    };
  }

  if (scope.type === "person") {
    const referencedHumanId = (contextRefs ?? []).find(
      (ref) => ref.kind === "human",
    )?.humanId;

    const humanId = scope.humanId ?? referencedHumanId;
    if (humanId) {
      return {
        filter: { kind: "human_id", humanId },
        isThisMeeting: false,
      };
    }
    return {
      filter: { kind: "session_ids", sessionIds: [] },
      isThisMeeting: false,
    };
  }

  if (scope.type === "organization") {
    const referencedOrgId = (contextRefs ?? []).find(
      (ref) => ref.kind === "organization",
    )?.organizationId;

    const orgId = scope.orgId ?? referencedOrgId;
    if (orgId) {
      return {
        filter: { kind: "org_id", orgId },
        isThisMeeting: false,
      };
    }
    return {
      filter: { kind: "session_ids", sessionIds: [] },
      isThisMeeting: false,
    };
  }

  return {
    filter: { kind: "none" },
    isThisMeeting: false,
  };
}
