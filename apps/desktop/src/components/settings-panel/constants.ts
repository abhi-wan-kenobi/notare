import {
  BellIcon,
  BlocksIcon,
  CalendarIcon,
  CreditCardIcon,
  FileTextIcon,
  SettingsIcon,
  UsersIcon,
} from "lucide-react";

export const data = {
  nav: [
    { name: "General", icon: SettingsIcon },
    { name: "Calendar", icon: CalendarIcon },
    { name: "Notifications", icon: BellIcon },
    { name: "Templates", icon: FileTextIcon },
    { name: "Extensions", icon: BlocksIcon },
    { name: "Team", icon: UsersIcon },
    { name: "Billing", icon: CreditCardIcon },
  ],
} as const;
