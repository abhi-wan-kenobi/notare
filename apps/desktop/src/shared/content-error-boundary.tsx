import { Component, type ErrorInfo, type ReactNode } from "react";

interface ContentErrorBoundaryProps {
  children: ReactNode;
  /**
   * When this value changes, any caught error is cleared and `children` is
   * given a fresh render - e.g. the current tab's unique id, so navigating
   * away from a crashed tab and back (or to a different tab entirely) always
   * gets a clean slate instead of staying stuck on the error card.
   */
  resetKey?: unknown;
}

interface ContentErrorBoundaryState {
  error: Error | null;
}

/**
 * A *local*, recoverable counterpart to `RootErrorBoundary` (../shared/root-error-boundary.tsx).
 *
 * `RootErrorBoundary` is the only error boundary mounted anywhere in the app
 * (see `main.tsx`) - it sits above the router, so a render error in *any*
 * single surface (a settings sub-page, a tab's content, a route) unmounts
 * the entire window down to that last-resort recovery screen: sidebar, tab
 * bar, everything, gone, with only a "Reload Notare" button to get back.
 * That is a disproportionate blast radius for a bug contained to one
 * component, and it also means an error can only ever be recovered by a full
 * reload - never by simply navigating away and back, even though several
 * surfaces (the tab content area in particular, see `main/body.tsx`) fully
 * unmount and remount their content on every navigation anyway.
 *
 * `ContentErrorBoundary` wraps a single content surface (a tab's content, a
 * routed window's outlet) so a render error there shows a small recoverable
 * card *in that surface only* - the rest of the window (sidebar, chrome,
 * other surfaces) stays intact and interactive. "Try again" simply clears
 * the caught error and re-renders `children` in place; passing a changing
 * `resetKey` (e.g. the active tab's id) additionally clears it automatically
 * whenever the caller already remounts that content for other reasons.
 *
 * Deliberately dependency-light like `RootErrorBoundary`: no lingui, no
 * router, no data hooks - a crash in any of those must not be able to take
 * this boundary down with it.
 */
export class ContentErrorBoundary extends Component<
  ContentErrorBoundaryProps,
  ContentErrorBoundaryState
> {
  state: ContentErrorBoundaryState = { error: null };

  static getDerivedStateFromError(
    error: unknown,
  ): ContentErrorBoundaryState {
    return {
      error: error instanceof Error ? error : new Error(String(error)),
    };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Content surface render error:", error, info.componentStack);
  }

  componentDidUpdate(prevProps: ContentErrorBoundaryProps) {
    if (this.state.error && prevProps.resetKey !== this.props.resetKey) {
      this.setState({ error: null });
    }
  }

  private handleRetry = () => {
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (!error) {
      return this.props.children;
    }

    return (
      <div
        data-testid="content-error-boundary"
        className="bg-background text-foreground flex h-full w-full items-center justify-center p-6"
      >
        <div className="border-border bg-card w-full max-w-md rounded-xl border p-6 shadow-xs">
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <p className="text-muted-foreground text-xs font-medium tracking-widest uppercase">
                Notare
              </p>
              <h1 className="text-base font-semibold">
                This section hit a problem
              </h1>
              <p className="text-muted-foreground text-sm leading-relaxed">
                Something here failed to render. The rest of the app is fine
                - try again, or switch away and back.
              </p>
            </div>

            <pre className="bg-muted text-muted-foreground max-h-40 overflow-auto rounded-md p-3 font-mono text-xs break-words whitespace-pre-wrap">
              {error.message || String(error)}
            </pre>

            <div>
              <button
                type="button"
                onClick={this.handleRetry}
                className="bg-primary text-primary-foreground hover:bg-primary/90 h-9 cursor-pointer rounded-md px-4 text-sm font-medium transition-colors"
              >
                Try again
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }
}
