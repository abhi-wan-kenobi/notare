import { StandardTabWrapper } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";
import { DateHeader } from "~/main2/home/date-header";
import { DailyNoteEditor } from "~/main2/home/note-editor";

type DailySummaryTab = Extract<Tab, { type: "daily_summary" }>;

export function TabContentDailySummary({ tab }: { tab: DailySummaryTab }) {
  return (
    <StandardTabWrapper>
      <div className="h-full overflow-auto">
        <DateHeader date={tab.id} />
        <DailyNoteEditor date={tab.id} />
      </div>
    </StandardTabWrapper>
  );
}
