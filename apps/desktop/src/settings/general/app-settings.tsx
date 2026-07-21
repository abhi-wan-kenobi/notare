import { Trans, useLingui } from "@lingui/react/macro";
import { useQueryClient } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { Loader2Icon } from "lucide-react";
import { type ReactNode, useEffect, useId, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

import {
  UPDATE_CHECK_QUERY_KEY,
  useDesktopUpdateControl,
} from "~/main/update-banner";
import {
  normalizeMeetingBarTheme,
  type MeetingBarTheme,
} from "~/meeting-float/window";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

interface SettingItem {
  value: boolean;
  onChange: (value: boolean) => void;
}

interface AppSettingsViewProps {
  autostart: SettingItem;
  autoStartScheduledMeetings: SettingItem;
  autoStopMeetings: SettingItem;
  floatingBar: SettingItem;
  showAppInDock: SettingItem;
  showTrayIcon: SettingItem;
  telemetryConsent: SettingItem;
}

export function AppSettingsView({
  autostart,
  autoStartScheduledMeetings,
  autoStopMeetings,
  floatingBar,
  showAppInDock,
  showTrayIcon,
  telemetryConsent,
}: AppSettingsViewProps) {
  return (
    <div className="flex flex-col gap-8">
      <section>
        <div className="flex flex-col gap-4">
          <SettingRow
            title={<Trans>Start Notare at login</Trans>}
            description={
              <Trans>Always ready without manually launching.</Trans>
            }
            checked={autostart.value}
            onChange={autostart.onChange}
          />
          <SettingRow
            title={<Trans>Share usage data</Trans>}
            description={
              <Trans>
                Send anonymous usage analytics to help improve Notare.
              </Trans>
            }
            checked={telemetryConsent.value}
            onChange={telemetryConsent.onChange}
          />
          <SettingRow
            title={<Trans>Show app in Dock</Trans>}
            description={
              <Trans>Show Notare in the Dock and app switcher.</Trans>
            }
            checked={showAppInDock.value}
            onChange={showAppInDock.onChange}
          />
          <SettingRow
            title={<Trans>Show tray icon</Trans>}
            description={
              <Trans>Keep Notare available from the menu bar.</Trans>
            }
            checked={showTrayIcon.value}
            onChange={showTrayIcon.onChange}
          />
        </div>
      </section>

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Meetings</Trans>
        </h2>
        <div className="flex flex-col gap-4">
          <SettingRow
            title={<Trans>Start when meeting begins</Trans>}
            description={
              <Trans>
                Automatically start listening when an event-backed note reaches
                its scheduled start time.
              </Trans>
            }
            checked={autoStartScheduledMeetings.value}
            onChange={autoStartScheduledMeetings.onChange}
          />
          <SettingRow
            title={<Trans>Stop when meeting ends</Trans>}
            description={
              <Trans>
                Automatically stop listening when the meeting app releases the
                microphone.
              </Trans>
            }
            checked={autoStopMeetings.value}
            onChange={autoStopMeetings.onChange}
          />
          <SettingRow
            title={<Trans>Show floating bar</Trans>}
            description={
              <Trans>Show the compact floating control while listening.</Trans>
            }
            checked={floatingBar.value}
            onChange={floatingBar.onChange}
          />
          <MeetingBarThemeGroup />
        </div>
      </section>

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Microphone</Trans>
        </h2>
        <div className="flex flex-col gap-4">
          <MicDenoiseRow />
        </div>
      </section>

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Transcription</Trans>
        </h2>
        <div className="flex flex-col gap-4">
          <LiveTranscriptionRow />
        </div>
      </section>

      <UpdatesSection />
    </div>
  );
}

/**
 * "Microphone noise suppression (experimental)" - the `mic_denoise` setting.
 * Self-contained (reads/writes the setting itself) so the surrounding
 * form-driven view stays untouched.
 */
export function MicDenoiseRow() {
  const micDenoise = useConfigValue("mic_denoise");
  const setMicDenoise = useSetSettingValue("mic_denoise");

  return (
    <SettingRow
      title={<Trans>Microphone noise suppression (experimental)</Trans>}
      description={
        <Trans>
          Reduce background noise on your microphone before transcription.
          Applies to new sessions only; recordings are always saved unprocessed.
        </Trans>
      }
      checked={micDenoise}
      onChange={setMicDenoise}
    />
  );
}

/**
 * "Live transcription" - the `live_transcription_enabled` setting. When off, a
 * meeting records audio only and transcribes the whole recording once when you
 * stop (lighter for long meetings; no live STT stream). Self-contained like
 * `MicDenoiseRow`.
 */
export function LiveTranscriptionRow() {
  const liveEnabled = useConfigValue("live_transcription_enabled");
  const setLiveEnabled = useSetSettingValue("live_transcription_enabled");

  return (
    <SettingRow
      title={<Trans>Live transcription</Trans>}
      description={
        <Trans>
          Transcribe in real time during a meeting. Turn this off to transcribe
          only after you stop — lighter for long meetings, and audio is always
          recorded either way. Applies to new sessions only.
        </Trans>
      }
      checked={liveEnabled}
      onChange={setLiveEnabled}
    />
  );
}

const MEETING_BAR_THEME_OPTIONS: ReadonlyArray<{
  value: MeetingBarTheme;
  title: ReactNode;
  description: ReactNode;
}> = [
  {
    value: "notare",
    title: <Trans>Notare</Trans>,
    description: (
      <Trans>The default glass bar with the live orb and captions.</Trans>
    ),
  },
  {
    value: "classic",
    title: <Trans>Classic</Trans>,
    description: (
      <Trans>The compact parchment bar with a dancing waveform.</Trans>
    ),
  },
];

/**
 * "Floating bar look" - the `meeting_bar_theme` setting (`notare` |
 * `classic`). Self-contained like `MicDenoiseRow` so the form-driven
 * `AppSettingsView` props stay unchanged. The Classic look is the React port
 * of the earlier native macOS floating bar (`FloatingBarView.swift`); the
 * Notare look is unchanged.
 */
export function MeetingBarThemeGroup() {
  const meetingBarTheme = useConfigValue("meeting_bar_theme");
  const setTheme = useSetSettingValue("meeting_bar_theme");
  const value = normalizeMeetingBarTheme(meetingBarTheme);
  const groupName = useId();

  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-medium">
        <Trans>Floating bar look</Trans>
      </h3>
      <div
        role="radiogroup"
        data-testid="meeting-bar-theme-group"
        className="flex flex-col gap-2"
      >
        {MEETING_BAR_THEME_OPTIONS.map((option) => {
          const selected = value === option.value;
          return (
            <label
              key={option.value}
              className={cn([
                "flex cursor-pointer items-start gap-3 rounded-lg border p-3",
                "transition-colors duration-(--motion-duration-state)",
                selected
                  ? "border-primary/60 bg-accent/40"
                  : "border-border hover:bg-accent/20",
              ])}
            >
              <input
                type="radio"
                name={groupName}
                value={option.value}
                checked={selected}
                onChange={() => setTheme(option.value)}
                className="accent-primary mt-0.5 shrink-0"
              />
              <span className="flex flex-1 flex-col gap-1">
                <span className="text-sm font-medium">{option.title}</span>
                <span className="text-muted-foreground text-xs">
                  {option.description}
                </span>
              </span>
            </label>
          );
        })}
      </div>
    </div>
  );
}

/**
 * Settings -> App -> Updates: current version, a manual "Check for updates"
 * and the live update state with its action. Shares poll/download/install
 * state with the sidebar pill via `useDesktopUpdateControl` (both observe
 * the same react-query key, so two mounts never double-poll).
 */
export function UpdatesSection() {
  const { t } = useLingui();
  const update = useDesktopUpdateControl();
  const queryClient = useQueryClient();
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [hasChecked, setHasChecked] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getVersion()
      .then((version) => {
        if (!cancelled) {
          setAppVersion(version);
        }
      })
      .catch(() => {
        // Not running under Tauri (tests/storybook): leave the row blank.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const checkForUpdates = async () => {
    setChecking(true);
    try {
      await queryClient.refetchQueries({
        queryKey: UPDATE_CHECK_QUERY_KEY,
        exact: true,
      });
      setHasChecked(true);
    } finally {
      setChecking(false);
    }
  };

  const stateText =
    update.status === "available"
      ? t`Version ${update.version ?? ""} is available.`
      : update.status === "downloading"
        ? update.progress === null
          ? t`Downloading version ${update.version ?? ""}…`
          : t`Downloading version ${update.version ?? ""}… ${Math.round(
              (update.progress ?? 0) * 100,
            )}%`
        : update.status === "ready"
          ? t`Version ${update.version ?? ""} is ready to install.`
          : update.status === "failed"
            ? (update.errorMessage ?? t`The update failed to download.`)
            : null;

  return (
    <section data-testid="updates-section">
      <h2 className="mb-4 font-sans text-lg font-semibold">
        <Trans>Updates</Trans>
      </h2>
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between gap-4">
          <div className="flex-1">
            <h3 className="mb-1 text-sm font-medium">
              <Trans>Current version</Trans>
            </h3>
            <p
              className="text-muted-foreground text-xs"
              data-testid="current-version"
            >
              {appVersion ? `Notare ${appVersion}` : "Notare"}
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            disabled={checking}
            onClick={() => void checkForUpdates()}
          >
            {checking ? (
              <>
                <Loader2Icon
                  aria-hidden
                  className="size-3.5 animate-spin motion-reduce:animate-none"
                />
                <Trans>Checking…</Trans>
              </>
            ) : (
              <Trans>Check for updates</Trans>
            )}
          </Button>
        </div>

        {update.status && update.version ? (
          <div className="flex items-center justify-between gap-4">
            <div className="flex-1">
              <h3 className="mb-1 text-sm font-medium">
                {update.status === "failed" ? (
                  <Trans>Update failed</Trans>
                ) : update.status === "ready" ? (
                  <Trans>Update ready</Trans>
                ) : (
                  <Trans>Update available</Trans>
                )}
              </h3>
              <p
                className="text-muted-foreground text-xs"
                data-testid="update-state"
              >
                {stateText}
              </p>
            </div>
            <Button
              variant={update.status === "ready" ? "default" : "outline"}
              size="sm"
              disabled={
                update.status === "downloading" ||
                update.downloadStarting ||
                update.installing
              }
              onClick={
                update.status === "ready"
                  ? update.installUpdate
                  : update.downloadUpdate
              }
            >
              {update.status === "ready" ? (
                <Trans>Restart to update</Trans>
              ) : update.status === "downloading" ? (
                <Trans>Downloading…</Trans>
              ) : update.status === "failed" ? (
                <Trans>Retry download</Trans>
              ) : (
                <Trans>Download</Trans>
              )}
            </Button>
          </div>
        ) : hasChecked && !checking ? (
          <p className="text-muted-foreground text-xs" data-testid="up-to-date">
            <Trans>You are up to date.</Trans>
          </p>
        ) : null}
      </div>
    </section>
  );
}

function SettingRow({
  title,
  description,
  checked,
  onChange,
}: {
  title: ReactNode;
  description: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  const titleId = useId();
  const descriptionId = useId();

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 id={titleId} className="mb-1 text-sm font-medium">
          {title}
        </h3>
        <p id={descriptionId} className="text-muted-foreground text-xs">
          {description}
        </p>
      </div>
      <Switch
        checked={checked}
        onCheckedChange={onChange}
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      />
    </div>
  );
}
