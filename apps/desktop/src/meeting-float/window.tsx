import { useLingui } from "@lingui/react/macro";
import {
  AppWindowIcon,
  CaptionsIcon,
  CaptionsOffIcon,
  SquareIcon,
} from "lucide-react";
import { useEffect, useState } from "react";

import {
  events as windowsEvents,
  type FloatingBarState,
} from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import { RecordingOrb } from "./orb";

/**
 * Webview-based floating meeting widget used on Windows/Linux.
 *
 * macOS renders a native NSPanel instead; on other platforms the Rust side of
 * the windows plugin creates a small always-on-top webview window pointing at
 * `/app/floating` and streams `FloatingBarStateEvent`s to it. Buttons talk
 * back to the main window through the same plugin events the native macOS bar
 * uses (`floatingBarStop`, `floatingBarOpenMain`, `floatingBarSettingsChange`).
 *
 * Design: docs/DESIGN-DIRECTION.md §3b — the orb inside a glass bar, with an
 * expandable caption bubble underneath. When the OS window could not be
 * created with transparency (known Windows limitation), the Rust side loads
 * `/app/floating?solid=1` and this component renders the solid-surface
 * variant instead of glass.
 */
export function FloatingBarWindow({ solid = false }: { solid?: boolean }) {
  const state = useFloatingBarState();

  useEffect(() => {
    if (solid) {
      return;
    }

    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, [solid]);

  // The widget colors come from the shared theme tokens; mirror the main
  // window's applied scheme onto this webview's root element.
  useEffect(() => {
    if (!state) {
      return;
    }

    document.documentElement.classList.toggle(
      "dark",
      state.colorScheme === "dark",
    );
  }, [state?.colorScheme]);

  if (!state) {
    return null;
  }

  return <FloatingBarContent state={state} solid={solid} />;
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

export function FloatingBarContent({
  state,
  solid = false,
}: {
  state: FloatingBarState;
  solid?: boolean;
}) {
  const { t } = useLingui();
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
      data-testid={solid ? "floating-bar-solid" : "floating-bar-glass"}
      className={cn([
        "text-foreground flex h-screen w-screen flex-col overflow-hidden",
        solid && "bg-background border-border rounded-xl border",
      ])}
    >
      <div
        data-tauri-drag-region
        className={cn([
          "group/bar flex h-[52px] shrink-0 items-center gap-2.5 px-2.5",
          solid
            ? "border-border/80 border-b"
            : "border-border/70 rounded-full border backdrop-blur-xl",
        ])}
        style={
          solid
            ? undefined
            : {
                backgroundColor: `hsl(var(--popover) / ${state.opacity})`,
              }
        }
      >
        <span data-tauri-drag-region className="pointer-events-none shrink-0">
          <RecordingOrb
            state={isError ? "error" : "listening"}
            amplitude={state.amplitude}
            size={32}
          />
        </span>
        {!isError && (
          <span
            aria-hidden
            className="bg-rec animate-orb-pulse shadow-glow-rec size-1.5 shrink-0 rounded-full motion-reduce:animate-none"
          />
        )}
        <span
          data-tauri-drag-region
          className="min-w-0 flex-1 truncate text-[13px] font-medium"
        >
          {state.title}
        </span>
        <div
          className={cn([
            "flex shrink-0 items-center gap-0.5",
            "opacity-60 transition-opacity duration-(--motion-duration-state)",
            "group-hover/bar:opacity-100 focus-within:opacity-100",
          ])}
        >
          {state.liveCaptionToggleVisible ? (
            <FloatingBarButton
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
            label={t`Open main window`}
            onClick={handleOpenMain}
          >
            <AppWindowIcon className="size-4" />
          </FloatingBarButton>
          <FloatingBarButton label={t`Stop recording`} onClick={handleStop}>
            <SquareIcon className="fill-rec text-rec size-3.5" />
          </FloatingBarButton>
        </div>
      </div>
      {captionsVisible ? (
        <FloatingBarCaptions
          bubbles={state.transcriptBubbles}
          lineCount={state.liveCaptionLineCount}
          solid={solid}
        />
      ) : null}
    </div>
  );
}

function FloatingBarButton({
  active = false,
  children,
  label,
  onClick,
}: {
  active?: boolean;
  children: React.ReactNode;
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
        "flex size-7 shrink-0 items-center justify-center rounded-full",
        "transition-colors duration-(--motion-duration-state)",
        "text-muted-foreground hover:bg-accent hover:text-foreground",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        active && "bg-accent text-foreground",
      ])}
    >
      {children}
    </button>
  );
}

const VISIBLE_CAPTION_BUBBLES = 3;

/**
 * Expandable caption bubble: last lines of the live transcript on a raised
 * glass surface. The newest line carries a cobalt live edge (glow =
 * information); older lines commit to muted text (§3a two-tone mechanics).
 */
function FloatingBarCaptions({
  bubbles,
  lineCount,
  solid,
}: {
  bubbles: FloatingBarState["transcriptBubbles"];
  lineCount: number;
  solid: boolean;
}) {
  const { t } = useLingui();
  const visibleBubbles = bubbles.slice(-VISIBLE_CAPTION_BUBBLES);

  return (
    <div
      data-testid="floating-bar-captions"
      className={cn([
        "flex min-h-0 flex-1 flex-col justify-end overflow-hidden px-3 pt-1 pb-1.5",
        solid
          ? "border-border/80 border-t"
          : "border-border/70 mt-1 rounded-[10px] border backdrop-blur-xl",
      ])}
      style={
        solid ? undefined : { backgroundColor: "hsl(var(--popover) / 0.92)" }
      }
    >
      {visibleBubbles.length === 0 ? (
        <p className="text-muted-foreground animate-orb-pulse text-xs motion-reduce:animate-none">
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
                  ? [
                      "border-primary/70 shrink-0 border-l-2 pl-2",
                      lineCount > 1 ? "line-clamp-2" : "truncate",
                    ]
                  : [
                      "text-muted-foreground truncate border-l-2 border-transparent pl-2",
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
