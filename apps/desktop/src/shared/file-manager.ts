import { platform } from "@tauri-apps/plugin-os";

/**
 * OS-correct name of the system file manager, for "Show in …" menu items.
 * Falls back to the generic "Files" when the platform is unknown (e.g. in
 * tests, where the Tauri API is unavailable).
 */
export function fileManagerName(): string {
  try {
    switch (platform()) {
      case "macos":
        return "Finder";
      case "windows":
        return "Explorer";
      default:
        return "Files";
    }
  } catch {
    return "Files";
  }
}
