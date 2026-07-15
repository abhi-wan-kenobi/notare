import { getStoredSettingValues, setSettingValues } from "~/settings/queries";
import type { SettingValues } from "~/settings/schema";

export async function configurePaidSettings(): Promise<void> {
  const { values } = await getStoredSettingValues();
  const updates: SettingValues = {};

  // No cloud tier in Notare: default to the local ("hyprnote") STT provider
  // and leave the model unset until the user downloads one. The upstream
  // hosted LLM ("hyprnote"/"Auto") is gone, so no LLM default is written.
  if (!values.current_stt_provider) {
    updates.current_stt_provider = "hyprnote";
  }

  await setSettingValues(updates);
}
