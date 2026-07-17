import { Trans } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { useState } from "react";

import { OnboardingButton } from "./shared";

import { useAppleCalendarSelection } from "~/calendar/components/apple/calendar-selection";
import { TroubleShootingLink } from "~/calendar/components/apple/permission";
import {
  type CalendarGroup,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import { SyncProvider, useSync } from "~/calendar/components/context";
import { GoogleDirectContent } from "~/calendar/components/google/content";
import { IcsContent } from "~/calendar/components/ics/content";
import { PROVIDERS } from "~/calendar/components/shared";
import { useEnabledCalendars } from "~/calendar/hooks";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { usePermission } from "~/shared/hooks/usePermissions";

// Outlook is intentionally absent: its only integration path was the deleted
// upstream Nango/Pro cloud, so the provider is hidden UI-wide (filtered out of
// PROVIDERS in ~/calendar/components/shared) until a direct BYO-OAuth path
// exists.
const GOOGLE_PROVIDER = PROVIDERS.find((provider) => provider.id === "google");
const ICS_PROVIDER = PROVIDERS.find((provider) => provider.id === "ics");

function getCalendarSelectionKey(groups: CalendarGroup[]) {
  return groups.length === 0
    ? "empty"
    : groups
        .map((group) => `${group.sourceName}:${group.calendars.length}`)
        .join("|");
}

function AppleCalendarList() {
  const { scheduleSync } = useSync();
  const { groups, handleRefresh, handleToggle, isLoading } =
    useAppleCalendarSelection();

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <CalendarSelection
      key={getCalendarSelectionKey(groups)}
      groups={groups}
      onToggle={handleToggle}
      onRefresh={handleRefresh}
      isLoading={isLoading}
      disableHoverTone
      className="border-border/45 bg-card/28 rounded-xl border shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
    />
  );
}

function AppleCalendarProvider({
  isAuthorized,
  isPending,
  onRequest,
  onTroubleshoot,
}: {
  isAuthorized: boolean;
  isPending: boolean;
  onRequest: () => void;
  onTroubleshoot: () => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      {isAuthorized ? (
        <AppleCalendarList />
      ) : (
        <OnboardingButton
          onClick={() => {
            onTroubleshoot();
            onRequest();
          }}
          disabled={isPending}
          className="border-border bg-card text-foreground hover:bg-accent flex h-full w-full items-center justify-center gap-3 border px-12 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] transition-all duration-150"
        >
          <img
            src="/assets/apple-calendar.png"
            alt=""
            aria-hidden="true"
            className="size-6 rounded-[4px] object-cover"
          />
          Apple
        </OnboardingButton>
      )}
    </div>
  );
}

function GoogleCalendarProvider() {
  if (!GOOGLE_PROVIDER) {
    return null;
  }

  // Direct (BYO OAuth client) integration — no Notare account involved.
  return (
    <div className="border-border/45 bg-card/28 w-full max-w-md rounded-xl border p-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150">
      <div className="mb-1 flex items-center gap-2">
        {GOOGLE_PROVIDER.icon}
        <span className="text-md text-foreground font-normal">Google</span>
      </div>
      <GoogleDirectContent config={GOOGLE_PROVIDER} />
    </div>
  );
}

function IcsCalendarProvider() {
  if (!ICS_PROVIDER) {
    return null;
  }

  // Local imported .ics files — no account or network involved.
  return (
    <div className="border-border/45 bg-card/28 w-full max-w-md rounded-xl border p-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150">
      <div className="mb-1 flex items-center gap-2">
        {ICS_PROVIDER.icon}
        <span className="text-md text-foreground font-normal">
          {ICS_PROVIDER.displayName}
        </span>
      </div>
      <IcsContent config={ICS_PROVIDER} />
    </div>
  );
}

function CalendarSectionContent({ onContinue }: { onContinue: () => void }) {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");
  const isAuthorized = calendar.status === "authorized";
  const [showTroubleshooting, setShowTroubleshooting] = useState(false);
  const enabledCalendars = useEnabledCalendars();
  const hasConnectedCalendar = enabledCalendars.length > 0;

  const hasAnyConnected = hasConnectedCalendar || isAuthorized;

  return (
    <div className="flex flex-col gap-4">
      {hasAnyConnected ? (
        <>
          {isMacos && (
            <AppleCalendarProvider
              isAuthorized={isAuthorized}
              isPending={calendar.isPending}
              onRequest={calendar.request}
              onTroubleshoot={() => setShowTroubleshooting(true)}
            />
          )}
          <div className="flex flex-wrap items-center gap-4">
            <GoogleCalendarProvider />
            <IcsCalendarProvider />
          </div>
          {hasConnectedCalendar && (
            <OnboardingButton onClick={onContinue}>
              <Trans>Continue</Trans>
            </OnboardingButton>
          )}
        </>
      ) : (
        // for the case when the user has no connected calendars yet we show the calendars in a row
        <div className="flex flex-wrap items-stretch gap-4">
          {isMacos && (
            <AppleCalendarProvider
              isAuthorized={isAuthorized}
              isPending={calendar.isPending}
              onRequest={calendar.request}
              onTroubleshoot={() => setShowTroubleshooting(true)}
            />
          )}

          <GoogleCalendarProvider />
          <IcsCalendarProvider />
        </div>
      )}

      {showTroubleshooting && !isAuthorized && (
        <TroubleShootingLink
          onRequest={calendar.request}
          onReset={calendar.reset}
          onOpen={calendar.open}
          isPending={calendar.isPending}
          className="text-muted-foreground text-sm"
        />
      )}
    </div>
  );
}

export function CalendarSection({ onContinue }: { onContinue: () => void }) {
  return (
    <SyncProvider>
      <CalendarSectionContent onContinue={onContinue} />
    </SyncProvider>
  );
}
