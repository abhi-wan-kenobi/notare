import { CalendarSidebarContent } from "~/calendar/components/sidebar";

export function CalendarNav() {
  return (
    <div className="flex h-full flex-col overflow-y-auto px-3 py-2">
      <CalendarSidebarContent />
    </div>
  );
}
