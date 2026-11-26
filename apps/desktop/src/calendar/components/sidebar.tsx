import { useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { ChevronRight, EyeOffIcon } from "lucide-react";
import { useMemo } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionHeader,
  AccordionItem,
  AccordionTriggerPrimitive,
} from "@hypr/ui/components/ui/accordion";
import { cn } from "@hypr/utils";

import { AppleCalendarSelection } from "./apple/calendar-selection";
import { AccessPermissionRow, TroubleShootingLink } from "./apple/permission";
import { GoogleDirectContent } from "./google/content";
import { IcsContent } from "./ics/content";
import { OutlookDirectContent } from "./outlook/content";
import { type CalendarProvider, PROVIDERS } from "./shared";

import {
  setCalendarProviderHidden,
  useHiddenCalendarProviders,
} from "~/calendar/hidden-providers";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { usePermission } from "~/shared/hooks/usePermissions";

function getProviderBadgeClassName(badge: string) {
  if (badge === "Beta") {
    return "text-xs font-medium text-muted-foreground";
  }

  return "rounded-full border border-border px-2 text-xs font-light text-muted-foreground";
}

function ProviderIcon({ provider }: { provider: CalendarProvider }) {
  return (
    <span className="flex size-5 shrink-0 items-center justify-center">
      {provider.icon}
    </span>
  );
}

function setProviderHidden(providerId: string, hidden: boolean) {
  void setCalendarProviderHidden(providerId, hidden).catch((error) => {
    console.error(`[calendar] failed to update hidden providers`, error);
  });
}

function HiddenProvidersRow({ providers }: { providers: CalendarProvider[] }) {
  const { t } = useLingui();
  const names = providers.map((provider) => provider.displayName).join(", ");

  return (
    <div
      data-testid="hidden-calendar-providers"
      className="text-muted-foreground -mx-2 flex items-center gap-2 px-2 py-3 text-xs"
    >
      <EyeOffIcon className="size-4 shrink-0" />
      <span className="min-w-0 flex-1 truncate" title={names}>
        {t`Hidden providers: ${names}`}
      </span>
      <button
        type="button"
        onClick={() =>
          providers.forEach((provider) => setProviderHidden(provider.id, false))
        }
        className="text-foreground hover:bg-accent shrink-0 rounded-full px-2 py-1 font-medium transition-colors"
      >
        {providers.length > 1 ? t`Show all` : t`Show`}
      </button>
    </div>
  );
}

export function CalendarSidebarContent() {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");

  const hiddenProviderIds = useHiddenCalendarProviders();
  const platformProviders = useMemo(
    () =>
      PROVIDERS.filter(
        (p) => p.platform === "all" || (p.platform === "macos" && isMacos),
      ),
    [isMacos],
  );
  const visibleProviders = useMemo(
    () => platformProviders.filter((p) => !hiddenProviderIds.includes(p.id)),
    [hiddenProviderIds, platformProviders],
  );
  const hiddenProviders = useMemo(
    () => platformProviders.filter((p) => hiddenProviderIds.includes(p.id)),
    [hiddenProviderIds, platformProviders],
  );
  const visibleProviderIds = useMemo(
    () => visibleProviders.map((provider) => provider.id),
    [visibleProviders],
  );

  return (
    <>
      <Accordion
        // Remount when the visible set changes so every shown provider
        // starts open, including one that was just unhidden.
        key={visibleProviderIds.join("|")}
        type="multiple"
        defaultValue={visibleProviderIds}
      >
        {visibleProviders.map((provider) =>
          provider.disabled ? (
            <div
              key={provider.id}
              className="-mx-2 flex items-center gap-2 px-2 py-3 opacity-50"
            >
              <ProviderIcon provider={provider} />
              <span className="text-sm font-medium">
                {provider.displayName}
              </span>
              {provider.badge && (
                <span className={getProviderBadgeClassName(provider.badge)}>
                  {provider.badge}
                </span>
              )}
            </div>
          ) : (
            <ProviderAccordionItem
              key={provider.id}
              provider={provider}
              calendar={calendar}
            />
          ),
        )}
      </Accordion>
      {hiddenProviders.length > 0 && (
        <HiddenProvidersRow providers={hiddenProviders} />
      )}
    </>
  );
}

function ProviderAccordionItem({
  provider,
  calendar,
}: {
  provider: CalendarProvider;
  calendar: ReturnType<typeof usePermission>;
}) {
  const { t } = useLingui();
  const providerMenuItems = useMemo(
    () => [
      {
        id: `hide-${provider.id}`,
        text: t`Hide ${provider.displayName}`,
        action: () => setProviderHidden(provider.id, true),
      },
    ],
    [provider.displayName, provider.id, t],
  );
  const showProviderMenu = useNativeContextMenu(providerMenuItems);

  return (
    <AccordionItem value={provider.id} className="group/provider border-none">
      <div
        onContextMenu={showProviderMenu}
        className="group/row hover:bg-accent relative -mx-2 grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-1 rounded-full px-2"
      >
        <AccordionHeader className="min-w-0">
          <AccordionTriggerPrimitive className="flex w-full min-w-0 items-center py-3 text-left text-sm font-medium transition-all hover:no-underline">
            <div className="flex min-w-0 items-center gap-2">
              <ProviderIcon provider={provider} />
              <span className="flex min-w-0 items-center gap-2">
                <span className="truncate text-sm font-medium">
                  {provider.displayName}
                </span>
                {provider.badge && (
                  <span className={getProviderBadgeClassName(provider.badge)}>
                    {provider.badge}
                  </span>
                )}
              </span>
            </div>
          </AccordionTriggerPrimitive>
        </AccordionHeader>

        <button
          type="button"
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            setProviderHidden(provider.id, true);
          }}
          className="text-muted-foreground hover:bg-accent hover:text-foreground shrink-0 rounded-full p-1 opacity-0 transition-all group-focus-within/row:opacity-100 group-hover/row:opacity-100"
          aria-label={t`Hide ${provider.displayName}`}
          title={t`Hide ${provider.displayName}`}
        >
          <EyeOffIcon className="size-4" />
        </button>

        <ChevronRight
          className={cn([
            "text-muted-foreground size-4 shrink-0 transition-transform duration-200",
            "group-data-[state=open]/provider:rotate-90",
          ])}
        />
      </div>
      <AccordionContent className="pb-3">
        {provider.id === "apple" && (
          <div className="flex flex-col gap-3">
            {calendar.status !== "authorized" ? (
              <AccessPermissionRow
                title={t`Calendar`}
                status={calendar.status}
                isPending={calendar.isPending}
                onOpen={calendar.open}
                onRequest={calendar.request}
                onReset={calendar.reset}
              />
            ) : (
              <AppleCalendarSelection
                leftAction={
                  <TroubleShootingLink
                    isPending={calendar.isPending}
                    onOpen={calendar.open}
                    onRequest={calendar.request}
                    onReset={calendar.reset}
                  />
                }
              />
            )}
          </div>
        )}
        {provider.id === "google" && <GoogleDirectContent config={provider} />}
        {provider.id === "outlook" && (
          <OutlookDirectContent config={provider} />
        )}
        {provider.ics && <IcsContent config={provider} />}
      </AccordionContent>
    </AccordionItem>
  );
}
