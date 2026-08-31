import { useMutation } from "@tanstack/react-query";

import {
  hardDeleteAllTrashedSessions,
  hardDeleteSession,
  loadTrashedSessionData,
  restoreDeletedSession,
} from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";
import { useUndoDelete } from "~/store/zustand/undo-delete";

/**
 * Trash view mutations. The list itself is a tinybase live query
 * (`useTrashedSessions` in `session/queries.ts`), so it re-renders on its own
 * when a row is restored or purged - these hooks only wrap the actions.
 *
 * Restore deliberately has no confirmation: deleting elsewhere already offers
 * a 5s undo, so the Trash row's Restore is the second chance and must be one
 * click. Hard delete is the opposite - irreversible, so it confirms.
 */
export function useRestoreTrashedSession() {
  const invalidateResource = useTabs((state) => state.invalidateResource);

  return useMutation({
    mutationFn: async (sessionId: string) => {
      const data = await loadTrashedSessionData(sessionId);
      if (!data) return false;

      await restoreDeletedSession(data);
      return true;
    },
    onSuccess: (restored, sessionId) => {
      if (restored) {
        invalidateResource("sessions", sessionId);
      }
    },
    onError: (error) => {
      console.error("[trash] failed to restore session", error);
    },
  });
}

export function useHardDeleteTrashedSession() {
  const clearDeletion = useUndoDelete((state) => state.clearDeletion);
  const invalidateResource = useTabs((state) => state.invalidateResource);

  return useMutation({
    mutationFn: (sessionId: string) => hardDeleteSession(sessionId),
    onSuccess: (_deleted, sessionId) => {
      // A pending undo for this session would otherwise fire its folder
      // cleanup against an already-purged row on timeout.
      clearDeletion(sessionId);
      invalidateResource("sessions", sessionId);
    },
    onError: (error) => {
      console.error("[trash] failed to delete session permanently", error);
    },
  });
}

export function useEmptyTrash() {
  return useMutation({
    mutationFn: () => hardDeleteAllTrashedSessions(),
    onSuccess: () => {
      // Drop any pending undos for sessions that were just purged.
      for (const sessionId of Object.keys(
        useUndoDelete.getState().pendingDeletions,
      )) {
        useUndoDelete.getState().clearDeletion(sessionId);
      }
    },
    onError: (error) => {
      console.error("[trash] failed to empty trash", error);
    },
  });
}
