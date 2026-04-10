import { CommandLineSettings } from "./command-line";

import { SettingsPageTitle } from "~/settings/page-title";

export function DeveloperSettings() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title="Developer" />
      <CommandLineSettings />
    </div>
  );
}
