import clsx from "clsx";
import SettingsPanel from "@/components/settings-panel";
import { NewNoteButton } from "@/components/toolbar/new-note-button";
import { BackButton } from "@/components/toolbar/back-button";
import { type OsType, type as getOsType } from "@tauri-apps/plugin-os";
import { useEffect, useState } from "react";
import { SearchBar, SearchIconButton, SearchPalette } from "../search";
import { RightSidePanelButton } from "./right-side-panel-button";

export default function Toolbar() {
  const [osType, setOsType] = useState<OsType>("macos");

  useEffect(() => {
    async function fetchOsType() {
      try {
        const os = getOsType(); // Returns "Linux", "Windows_NT", "Darwin"
        setOsType(os);
      } catch (error) {
        console.error("Failed to get OS type:", error);
      }
    }
    fetchOsType();
  }, []);

  return (
    <>
      <header
        className={clsx([
          "flex w-full items-center justify-between",
          "border-b border-border bg-neutral-100",
          "min-h-11 p-1 px-2",
        ])}
        data-tauri-drag-region
      >
        {/* TODO: This is a poor way for implementing just for macOS */}
        <div className="w-40" data-tauri-drag-region>
          {osType === "macos" && <BackButton />}
        </div>

        <SearchBar />

        <div
          className="flex w-40 items-center justify-end gap-1"
          data-tauri-drag-region
        >
          <SearchIconButton />
          <NewNoteButton />
          <RightSidePanelButton />
          <SettingsPanel />
        </div>
      </header>

      <SearchPalette />
    </>
  );
}
