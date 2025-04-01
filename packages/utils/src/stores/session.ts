import { create as mutate } from "mutative";
import { createStore } from "zustand";

import { commands as dbCommands, type Session } from "@hypr/plugin-db";
import pDebounce from "p-debounce";

type State = {
  session: Session;
  showRaw: boolean;
};

type Actions = {
  get: () => State & Actions;
  setShowRaw: (showRaw: boolean) => void;
  updateTitle: (title: string) => void;
  updateRawNote: (note: string) => void;
  updateEnhancedNote: (note: string) => void;
  persistSession: (now?: boolean) => Promise<void>;
};

export type SessionStore = ReturnType<typeof createSessionStore>;

export const createSessionStore = (session: Session) => {
  return createStore<State & Actions>((set, get) => ({
    session,
    showRaw: !session.enhanced_memo_html,
    get,
    setShowRaw: (showRaw: boolean) => {
      set((state) =>
        mutate(state, (draft) => {
          draft.showRaw = showRaw;
        })
      );
    },
    updateTitle: (title: string) => {
      set((state) => {
        if (!state.session) {
          return state;
        }

        state.persistSession();
        return mutate(state, (draft) => {
          draft.session.title = title;
        });
      });
    },
    updateRawNote: (note: string) => {
      set((state) => {
        if (!state.session) {
          return state;
        }

        state.persistSession();
        return mutate(state, (draft) => {
          if (!draft.session) {
            return;
          }

          draft.session.raw_memo_html = note;
        });
      });
    },
    updateEnhancedNote: (note: string) => {
      set((state) => {
        if (!state.session) {
          return state;
        }

        state.persistSession();
        return mutate(state, (draft) => {
          if (!draft.session) {
            return;
          }

          draft.showRaw = false;
          draft.session.enhanced_memo_html = note;
        });
      });
    },
    persistSession: async (now = false) => {
      const { session } = get();
      if (!session) {
        return;
      }

      const fn = now ? dbCommands.upsertSession : pDebounce(() => dbCommands.upsertSession(session), 250);
      await fn(session);
    },
  }));
};
