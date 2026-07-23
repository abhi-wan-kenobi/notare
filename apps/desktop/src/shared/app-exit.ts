import { listen } from "@tauri-apps/api/event";

import { commands as store2Commands } from "@hypr/plugin-store2";
import { commands as listenerCommands } from "@hypr/plugin-transcription";

import { flushDatabaseWrites } from "~/db/write-queue";
import { commands } from "~/types/tauri.gen";

const APP_EXIT_REQUESTED_EVENT = "app-exit-requested";

let exitInProgress = false;

export async function initializeAppExitFlush(): Promise<void> {
  await listen(APP_EXIT_REQUESTED_EVENT, () => {
    if (exitInProgress) {
      return;
    }

    exitInProgress = true;
    void flushAndExit();
  });
}

async function flushAndExit(): Promise<void> {
  try {
    // DATA-LOSS FIX (macOS recording survives app restart): an in-flight
    // recording lives in the backend RootActor and must be finalized to disk
    // BEFORE we honor the exit, or a restart drops it. Finalize FIRST so the
    // finalized session's DB writes are included in the flush below. Idempotent
    // (no-op when nothing is recording).
    await listenerCommands.stopCapture();
  } catch (error) {
    console.error("Failed to finalize active recording before exit", error);
  }

  try {
    await Promise.all([flushDatabaseWrites(), store2Commands.save()]);
  } catch (error) {
    console.error("Failed to flush application data before exit", error);
  } finally {
    await commands.completeAppExit();
  }
}
