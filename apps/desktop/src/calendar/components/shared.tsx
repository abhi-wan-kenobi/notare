import { Icon } from "@iconify-icon/react";
import type { ReactNode } from "react";

import { OutlookIcon } from "@hypr/ui/components/icons/outlook";

export type CalendarProvider = {
  disabled: boolean;
  id: string;
  displayName: string;
  icon: ReactNode;
  badge?: string | null;
  platform?: "macos" | "all";
  docsPath: string;
  /** Legacy cloud (Nango) integration id — dead in Notare, kept for Outlook. */
  nangoIntegrationId?: string;
  /** Direct integration using the user's own OAuth client (no cloud). */
  direct?: boolean;
};

const _PROVIDERS = [
  {
    disabled: false,
    id: "apple",
    displayName: "Apple Calendar",
    badge: "",
    icon: (
      <img
        src="/assets/apple-calendar.png"
        alt="Apple Calendar"
        className="size-5 rounded-[4px] object-cover"
      />
    ),
    platform: "macos",
    docsPath: "https://docs.anarlog.so/calendar#apple-calendar",
    nangoIntegrationId: undefined,
  },
  {
    disabled: false,
    id: "google",
    displayName: "Google",
    badge: "",
    icon: <Icon icon="logos:google-calendar" width={16} height={16} />,
    platform: "all",
    docsPath:
      "https://github.com/abhi-wan-kenobi/notare/blob/main/docs/GOOGLE-CALENDAR.md",
    nangoIntegrationId: undefined,
    direct: true,
  },
  {
    disabled: false,
    id: "outlook",
    displayName: "Outlook",
    badge: "",
    icon: <OutlookIcon size={16} />,
    platform: "all",
    docsPath: "https://docs.anarlog.so/calendar#outlook-calendar",
    nangoIntegrationId: "outlook",
  },
] as const satisfies readonly CalendarProvider[];

export const PROVIDERS = [..._PROVIDERS];
