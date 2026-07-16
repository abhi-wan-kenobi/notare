import { Component, type ErrorInfo, type ReactNode } from "react";

interface RootErrorBoundaryProps {
  children: ReactNode;
}

interface RootErrorBoundaryState {
  error: Error | null;
}

/**
 * Last-resort error boundary mounted at the very root of the app (above the
 * router, providers, and stores). If anything throws during render — even on
 * the very first render — the user gets a minimal recovery screen instead of
 * a blank window.
 *
 * Deliberately dependency-free: no lingui, no router, no UI-kit components.
 * Any of those crashing is exactly what this boundary must survive. Styling
 * uses only the design-token utility classes from the statically imported
 * global stylesheets ("Cobalt on graphite" palette).
 */
export class RootErrorBoundary extends Component<
  RootErrorBoundaryProps,
  RootErrorBoundaryState
> {
  state: RootErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): RootErrorBoundaryState {
    return {
      error: error instanceof Error ? error : new Error(String(error)),
    };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Unrecoverable render error:", error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (!error) {
      return this.props.children;
    }

    return (
      <div
        data-testid="root-error-boundary"
        className="bg-background text-foreground flex h-screen w-screen items-center justify-center p-6"
      >
        <div className="border-border bg-card w-full max-w-md rounded-xl border p-6 shadow-xs">
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <p className="text-muted-foreground text-xs font-medium tracking-widest uppercase">
                Notare
              </p>
              <h1 className="text-base font-semibold">Something went wrong</h1>
              <p className="text-muted-foreground text-sm leading-relaxed">
                The app hit an error it could not recover from. Reloading
                usually fixes it.
              </p>
            </div>

            <pre className="bg-muted text-muted-foreground max-h-40 overflow-auto rounded-md p-3 font-mono text-xs break-words whitespace-pre-wrap">
              {error.message || String(error)}
            </pre>

            <div>
              <button
                type="button"
                onClick={() => {
                  window.location.reload();
                }}
                className="bg-primary text-primary-foreground hover:bg-primary/90 h-9 cursor-pointer rounded-md px-4 text-sm font-medium transition-colors"
              >
                Reload Notare
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }
}
