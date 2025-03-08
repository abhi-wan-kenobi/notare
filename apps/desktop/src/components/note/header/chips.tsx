import { EventChip } from "./event-chip";
import { ParticipantsChip } from "./participants-chip";
import { TagChip } from "./tag-chip";

export default function Chips() {
  return (
    <div className="-mx-1.5 flex flex-row items-center sm:px-8 px-4 pb-4 pt-1 overflow-x-auto scrollbar-none whitespace-nowrap">
      <EventChip />
      <div className="mx-1 h-4 w-px flex-shrink-0 bg-border" />
      <ParticipantsChip />
      <div className="mx-1 h-4 w-px flex-shrink-0 bg-border" />
      <TagChip />
    </div>
  );
}
