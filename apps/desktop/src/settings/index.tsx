import { Trans } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { type ReactNode, useEffect, useRef } from "react";

import { CalendarSidebarContent } from "~/calendar/components/sidebar";
import { AccountSettingsView } from "~/settings/account";
import { LLM } from "~/settings/ai/llm";
import { STT } from "~/settings/ai/stt";
import { SectionLabel } from "~/settings/components";
import { SettingsDictation } from "~/settings/dictation";
import {
  SettingsApp,
  SettingsNotifications,
  SettingsPermissions,
} from "~/settings/general";
import { SettingsGmail } from "~/settings/gmail";
import { resolveSettingsHub } from "~/settings/hubs";
import { SettingsMcp } from "~/settings/mcp";
import { SettingsPageTitle } from "~/settings/page-title";
import { SettingsPersonalization } from "~/settings/personalization";
import { SettingsRecipes } from "~/settings/recipes";
import { SettingsSync } from "~/settings/sync";
import { SettingsWebhook } from "~/settings/webhook";
import { StandardContentWrapper } from "~/shared/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

const HUBS = [
  { id: "general", title: <Trans>General &amp; System</Trans> },
  { id: "ai", title: <Trans>AI &amp; Speech</Trans> },
  { id: "integrations", title: <Trans>Integrations &amp; Sync</Trans> },
  { id: "account", title: <Trans>Account &amp; Storage</Trans> },
] as const;

export function TabContentSettings({
  tab,
}: {
  tab: Extract<Tab, { type: "settings" }>;
}) {
  return (
    <StandardContentWrapper>
      <SettingsView tab={tab} />
    </StandardContentWrapper>
  );
}

function SettingsView({ tab }: { tab: Extract<Tab, { type: "settings" }> }) {
  const isMacos = platform() === "macos";
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );
  const containerRef = useRef<HTMLDivElement>(null);
  const selected = resolveSettingsHub(tab.state?.tab);
  const hub = HUBS.find((candidate) => candidate.id === selected) ?? HUBS[0];
  const activeSection = tab.state?.section;

  useEffect(() => {
    if (!activeSection || !containerRef.current) return;
    const target = containerRef.current.querySelector(
      `[data-section="${activeSection}"]`,
    );
    if (target) {
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }, [activeSection, hub.id]);

  return (
    <main
      data-settings-content
      className="bg-background flex w-full flex-1 flex-col overflow-hidden"
    >
      <div
        ref={containerRef}
        className="scroll-fade-y scrollbar-hide h-full w-full flex-1 overflow-y-auto px-8 py-7"
      >
        <div className="mx-auto flex w-full max-w-4xl flex-col gap-7">
          <header>
            <div className="text-muted-foreground mb-2 text-[10px] font-semibold tracking-[0.09em] uppercase">
              <Trans>Preferences</Trans>
            </div>
            <SettingsPageTitle title={<Trans>Settings</Trans>} />
            <p className="text-muted-foreground mt-3 text-sm">
              <Trans>
                A few thoughtful controls for your workspace on this device.
              </Trans>
            </p>
            <nav
              aria-label="Settings hubs"
              className="border-border/70 mt-6 flex flex-wrap gap-1 border-b pb-2"
            >
              {HUBS.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  aria-pressed={item.id === hub.id}
                  onClick={() => updateSettingsTabState(tab, { tab: item.id })}
                  className={
                    item.id === hub.id
                      ? "bg-muted text-foreground rounded-md px-3 py-2 text-sm font-medium"
                      : "text-muted-foreground hover:text-foreground rounded-md px-3 py-2 text-sm"
                  }
                >
                  {item.title}
                </button>
              ))}
            </nav>
          </header>
          <div className="flex flex-col gap-8">
            {hub.id === "general" && (
              <>
                <SettingsSection
                  title={<Trans>Appearance &amp; preferences</Trans>}
                  data-section="app"
                >
                  <SettingsApp />
                </SettingsSection>
                <SettingsSection
                  title={<Trans>Notifications</Trans>}
                  data-section="notifications"
                >
                  <SettingsNotifications />
                </SettingsSection>
                <SettingsSection
                  title={<Trans>Local system</Trans>}
                  data-section="permissions"
                >
                  {isMacos && <SettingsPermissions />}
                  <div data-section="personalization">
                    <SettingsPersonalization />
                  </div>
                  <div data-section="dictation">
                    <SettingsDictation />
                  </div>
                </SettingsSection>
              </>
            )}
            {hub.id === "ai" && (
              <>
                <SettingsSection
                  title={<Trans>Speech recognition</Trans>}
                  data-section="transcription"
                >
                  <STT />
                </SettingsSection>
                <SettingsSection
                  title={<Trans>Language models</Trans>}
                  data-section="intelligence"
                >
                  <LLM />
                </SettingsSection>
                <SettingsSection
                  title={<Trans>Recipes</Trans>}
                  data-section="recipes"
                >
                  <SettingsRecipes />
                </SettingsSection>
              </>
            )}
            {hub.id === "integrations" && (
              <>
                <SettingsSection
                  title={<Trans>Calendar providers</Trans>}
                  data-section="calendar"
                >
                  <CalendarSidebarContent returnTo="settings" />
                </SettingsSection>
                <SettingsSection
                  title={<Trans>Connected services</Trans>}
                  data-section="gmail"
                >
                  <SettingsGmail />
                  <div data-section="webhook">
                    <SettingsWebhook />
                  </div>
                </SettingsSection>
                <SettingsSection
                  title={<Trans>Local integrations</Trans>}
                  data-section="mcp"
                >
                  <SettingsMcp />
                  <div data-section="sync">
                    <SettingsSync />
                  </div>
                </SettingsSection>
              </>
            )}
            {hub.id === "account" && (
              <div data-section="account">
                <AccountSettingsView />
              </div>
            )}
          </div>
        </div>
      </div>
    </main>
  );
}

function SettingsSection({
  title,
  children,
  "data-section": dataSection,
}: {
  title: ReactNode;
  children: ReactNode;
  "data-section"?: string;
}) {
  return (
    <section data-section={dataSection} className="flex flex-col gap-3">
      <SectionLabel>{title}</SectionLabel>
      <div className="flex flex-col gap-4">{children}</div>
    </section>
  );
}

export { CalendarSidebarContent as CalendarProvidersSection } from "~/calendar/components/sidebar";
export {
  SectionLabel,
  SettingsExpander,
  SettingsRow,
  SettingsToggleRow,
} from "./components";
