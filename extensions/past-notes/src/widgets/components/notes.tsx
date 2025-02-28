import { formatTime } from "../../utils";
import { type Session } from "@hypr/plugin-db";

export default function Notes({ notes }: { notes: Session[] }) {
  if (!notes) {
    return null;
  }

  return (
    <div className="flex flex-col space-y-4">
      {notes.map((note, index) => (
        <div
          key={index}
          className="flex flex-col bg-white rounded-lg p-3 shadow-sm border border-neutral-100"
        >
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium">{note.title}</span>
          </div>
        </div>
      ))}
    </div>
  );
}
