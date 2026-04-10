import { ComposerSettings } from "./composer";
import { V1p1PreviewToggle } from "./v1p1-preview-toggle";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsLab() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title="General" />

      <div>
        <h2 className="mb-4 font-serif text-lg font-semibold">Preview</h2>
        <div className="flex flex-col gap-6">
          <ComposerSettings />
          <V1p1PreviewToggle />
        </div>
      </div>
    </div>
  );
}
