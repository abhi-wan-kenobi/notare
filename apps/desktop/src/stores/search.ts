import { commands as dbCommands, type Event, type Human, type Organization, type Session } from "@hypr/plugin-db";
import { createStore } from "zustand";

export type SearchMatch = {
  type: "session";
  item: Session;
} | {
  type: "event";
  item: Event;
} | {
  type: "human";
  item: Human;
} | {
  type: "organization";
  item: Organization;
};

type State = {
  previous?: URL;
  query: string;
  matches: SearchMatch[];
  searchInputRef: React.RefObject<HTMLInputElement> | null;
};

type Actions = {
  setQuery: (query: string) => void;
  clearSearch: () => void;
  focusSearch: () => void;
  setSearchInputRef: (ref: React.RefObject<HTMLInputElement>) => void;
};

export type SearchStore = ReturnType<typeof createSearchStore>;

export const createSearchStore = (userId: string) => {
  return createStore<State & Actions>((set, get) => ({
    query: "",
    matches: [],
    searchInputRef: null,
    setQuery: async (query: string) => {
      const [sessions, humans, organizations] = await Promise.all([
        dbCommands.listSessions({ type: "search", query, limit: 5, user_id: userId }),
        dbCommands.listHumans({ search: [3, query] }),
        dbCommands.listOrganizations({ search: [3, query] }),
      ]);

      const matches: SearchMatch[] = [
        ...sessions.map((session) => ({
          type: "session" as const,
          item: session,
        })),
        ...humans.map((human) => ({
          type: "human" as const,
          item: human,
        })),
        ...organizations.map((organization) => ({
          type: "organization" as const,
          item: organization,
        })),
      ];

      const url = new URL(window.location.href);
      if (url.pathname.includes("note")) {
        set({ previous: url });
      }

      if (query === "") {
        handleEmpty(get);
      }

      set({ query, matches });
    },
    clearSearch: () => {
      const { searchInputRef } = get();
      searchInputRef?.current?.blur();

      handleEmpty(get);
      set({ query: "" });
    },
    focusSearch: () => {
      setTimeout(() => {
        get().searchInputRef?.current?.focus();
      }, 10);
    },
    setSearchInputRef: (ref: React.RefObject<HTMLInputElement>) => set({ searchInputRef: ref }),
  }));
};

const handleEmpty = (get: () => State) => {
  const { previous } = get();
  if (previous) {
    window.history.pushState({}, "", previous.pathname);
  }
};
