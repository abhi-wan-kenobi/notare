import { useLingui } from "@lingui/react/macro";
import { AppWindowIcon, CaptionsIcon, CaptionsOffIcon, SquareIcon } from "lucide-react";
import { useEffect, useState } from "react";

import {
  events as windowsEvents,
  type FloatingBarState,
} from "@hypr/plugin-windows";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

/**
 * Webview-based floating meeting bar used on Windows/Linux.
 *
 * macOS renders a native NSPanel instead; on other platforms the Rust side of
 * the windows plugin creates a small always-on-top webview window pointing at
 * `/app/floating` and streams `FloatingBarStateEvent`s to it. Buttons talk
 * back to the main window through the same plugin events the native macOS bar
 * uses (`floatingBarStop`, `floatingBarOpenMain`, `floatingBarSettingsChange`).
 */
export function FloatingBarWindow() {
  const state = useFloatingBarState();

  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, []);

  if (!state) {
    return null;
  }

  return <FloatingBarContent state={state} />;
}

function useFloatingBarState() {
  const [state, setState] = useState<FloatingBarState | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void windowsEvents.floatingBarStateEvent
      .listen((event) => {
        if (!cancelled) {
          setState(event.payload.state);
        }
      })
      .then((next) => {
        if (cancelled) {
          next();
          return;
        }

        unlisten = next;
        // Ask the main window to push the current state: this window loads
        // after recording starts, so the initial updates were already sent.
        void windowsEvents.floatingBarReady.emit({});
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return state;
}

function FloatingBarContent({ state }: { state: FloatingBarState }) {
  const { t } = useLingui();
  const isDark = state.colorScheme === "dark";
  const isError = state.status === "error";
  const captionsVisible = !state.liveCaptionMinimized;

  const handleStop = () => {
    void windowsEvents.floatingBarStop.emit({});
  };
  const handleOpenMain = () => {
    void windowsEvents.floatingBarOpenMain.emit({});
  };
  const handleToggleCaptions = () => {
    void windowsEvents.floatingBarSettingsChange.emit({
      floatingBarOpacity: null,
      liveCaptionOpacity: null,
      liveCaptionWidth: null,
      liveCaptionLineCount: null,
      liveCaptionPosition: null,
      liveCaptionMinimized: captionsVisible,
    });
  };

  return (
    <div
      className={cn([
        "flex h-screen w-screen flex-col overflow-hidden rounded-xl border",
        isDark
          ? "border-white/10 text-neutral-100"
          : "border-black/10 text-neutral-900",
      ])}
      style={{
        backgroundColor: isDark
          ? `rgba(23, 23, 23, ${state.opacity})`
          : `rgba(250, 250, 250, ${state.opacity})`,
      }}
    >
      <div
        data-tauri-drag-region
        className="flex h-[52px] shrink-0 items-center gap-2 px-3"
      >
        <span
          data-tauri-drag-region
          className="pointer-events-none flex size-4 shrink-0 items-center justify-center"
        >
          <DancingSticks
            amplitude={state.amplitude}
            color={isError ? "#f59e0b" : "#ef4444"}
            height={16}
            width={16}
          />
        </span>
        <span
          data-tauri-drag-region
          className="min-w-0 flex-1 truncate text-xs font-medium"
        >
          {state.title}
        </span>
        {state.liveCaptionToggleVisible ? (
          <FloatingBarButton
            isDark={isDark}
            label={captionsVisible ? t`Hide captions` : t`Show captions`}
            onClick={handleToggleCaptions}
            active={captionsVisible}
          >
            {captionsVisible ? (
              <CaptionsIcon className="size-4" />
            ) : (
              <CaptionsOffIcon className="size-4" />
            )}
          </FloatingBarButton>
        ) : null}
        <FloatingBarButton
          isDark={isDark}
          label={t`Open main window`}
          onClick={handleOpenMain}
        >
          <AppWindowIcon className="size-4" />
        </FloatingBarButton>
        <FloatingBarButton
          isDark={isDark}
          label={t`Stop recording`}
          onClick={handleStop}
        >
          <SquareIcon className="size-3.5 fill-red-500 text-red-500" />
        </FloatingBarButton>
      </div>
      {captionsVisible ? (
        <FloatingBarCaptions
          bubbles={state.transcriptBubbles}
          lineCount={state.liveCaptionLineCount}
          isDark={isDark}
        />
      ) : null}
    </div>
  );
}

function FloatingBarButton({
  active = false,
  children,
  isDark,
  label,
  onClick,
}: {
  active?: boolean;
  children: React.ReactNode;
  isDark: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={active}
      onClick={onClick}
      className={cn([
        "flex size-7 shrink-0 items-center justify-center rounded-md transition-colors",
        isDark
          ? "text-neutral-300 hover:bg-white/10 hover:text-white"
          : "text-neutral-600 hover:bg-black/10 hover:text-black",
        active ? (isDark ? "bg-white/15 text-white" : "bg-black/10 text-black") : null,
      ])}
    >
      {children}
    </button>
  );
}

const VISIBLE_CAPTION_BUBBLES = 3;

function FloatingBarCaptions({
  bubbles,
  isDark,
  lineCount,
}: {
  bubbles: FloatingBarState["transcriptBubbles"];
  isDark: boolean;
  lineCount: number;
}) {
  const { t } = useLingui();
  const visibleBubbles = bubbles.slice(-VISIBLE_CAPTION_BUBBLES);

  return (
    <div
      data-testid="floating-bar-captions"
      className={cn([
        "flex min-h-0 flex-1 flex-col justify-end overflow-hidden border-t px-3 pt-1.5 pb-2",
        isDark ? "border-white/10" : "border-black/10",
      ])}
    >
      {visibleBubbles.length === 0 ? (
        <p
          className={cn([
            "text-xs",
            isDark ? "text-neutral-400" : "text-neutral-500",
          ])}
        >
          {t`Listening...`}
        </p>
      ) : (
        visibleBubbles.map((bubble, index) => {
          const isLatest = index === visibleBubbles.length - 1;

          return (
            <p
              key={bubble.id}
              className={cn([
                "text-xs leading-[22px]",
                isLatest
                  ? ["shrink-0", lineCount > 1 ? "line-clamp-2" : "truncate"]
                  : [
                      "truncate",
                      isDark ? "text-neutral-400" : "text-neutral-500",
                    ],
              ])}
            >
              <span className="font-medium">{bubble.speakerLabel}</span>
              {": "}
              {bubble.text}
            </p>
          );
        })
      )}
    </div>
  );
}
