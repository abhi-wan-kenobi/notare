import { Icon } from "@iconify-icon/react";
import { CalendarPlus } from "lucide-react";
import type { ReactNode } from "react";

import { OutlookIcon } from "@hypr/ui/components/icons/outlook";

export type CalendarProvider = {
  disabled: boolean;
  id: string;
  displayName: string;
  icon: ReactNode;
  badge?: string | null;
  platform?: "macos" | "all";
  docsPath?: string;
  /** Legacy cloud (Nango) integration id — dead in Notare, kept for Outlook. */
  nangoIntegrationId?: string;
  /** Direct integration using the user's own OAuth client (no cloud). */
  direct?: boolean;
  /** Local imported `.ics` calendar files (no network at all). */
  ics?: boolean;
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
    docsPath:
      "https://github.com/abhi-wan-kenobi/notare/blob/main/docs/OUTLOOK-CALENDAR.md",
    nangoIntegrationId: undefined,
    direct: true,
  },
  {
    disabled: false,
    id: "ics",
    displayName: "Calendar files (.ics)",
    badge: "",
    icon: <CalendarPlus size={16} />,
    platform: "all",
    docsPath:
      "https://github.com/abhi-wan-kenobi/notare/blob/main/docs/ICS-IMPORT.md",
    nangoIntegrationId: undefined,
    ics: true,
  },
] as const satisfies readonly CalendarProvider[];

export const PROVIDERS = _PROVIDERS;
