import type { SettingsTab } from "~/store/zustand/tabs";

export const SETTINGS_HUBS: readonly SettingsTab[] = [
  "general",
  "ai",
  "integrations",
  "account",
] as const;

export type SettingsHubId = SettingsTab;

export function resolveSettingsHub(
  tab: string | null | undefined,
): SettingsTab {
  switch (tab) {
    case "general":
    case "app":
    case "notifications":
    case "permissions":
    case "personalization":
    case "dictation":
      return "general";
    case "ai":
    case "intelligence":
    case "transcription":
    case "recipes":
      return "ai";
    case "integrations":
    case "calendar":
    case "sync":
    case "gmail":
    case "mcp":
    case "webhook":
      return "integrations";
    case "account":
    case "storage":
    case "todo":
    case "export":
    case "delete":
      return "account";
    default:
      return "general";
  }
}
