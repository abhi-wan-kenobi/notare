import { useLingui } from "@lingui/react/macro";
import { ask } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";

import { useIgnoredEvents } from "~/calendar/ignored-events";

/** Hides a whole recurring series app-wide, after the user confirms. */
export function useHideRecurringSeries() {
  const { t } = useLingui();
  const { ignoreSeries } = useIgnoredEvents();

  return useCallback(
    (seriesId: string | null | undefined) => {
      if (!seriesId) {
        return;
      }

      void ask(
        t`Hide every event in this recurring series? They stay in your calendar and are only hidden in Notare.`,
        {
          title: t`Hide recurring events`,
          kind: "warning",
          okLabel: t`Hide`,
          cancelLabel: t`Cancel`,
        },
      )
        .then((confirmed) => {
          if (confirmed) {
            ignoreSeries(seriesId);
          }
        })
        .catch((error) => {
          console.error("[calendar] failed to confirm hiding series", error);
        });
    },
    [ignoreSeries, t],
  );
}
