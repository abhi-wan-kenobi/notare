import { CommandLineSettings } from "./command-line";
import { DownloadButtons } from "./download-buttons";
import { V1p1PreviewToggle } from "./v1p1-preview-toggle";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsLab() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title="General" />

      <div>
        <h2 className="mb-4 font-serif text-lg font-semibold">Updates</h2>
        <div className="flex flex-col gap-6">
          <DownloadButtons />
        </div>
      </div>

      <CommandLineSettings />

      <div>
        <h2 className="mb-4 font-serif text-lg font-semibold">Preview</h2>
        <div className="flex flex-col gap-6">
          <V1p1PreviewToggle />
        </div>
      </div>
    </div>
  );
}
