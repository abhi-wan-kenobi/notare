function ordinalSuffix(day: number): string {
  if (day >= 11 && day <= 13) return "th";
  switch (day % 10) {
    case 1:
      return "st";
    case 2:
      return "nd";
    case 3:
      return "rd";
    default:
      return "th";
  }
}

function formatDateHeader(dateStr: string): string {
  const date = new Date(dateStr + "T00:00:00");
  const month = date.toLocaleDateString("en-US", { month: "long" });
  const day = date.getDate();
  return `${month} ${day}${ordinalSuffix(day)}`;
}

function getTodayString(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function DateHeader({ date, muted }: { date: string; muted?: boolean }) {
  const isToday = date === getTodayString();
  return (
    <div className="flex items-center gap-3 px-6 pt-6 pb-3">
      <h2
        className={
          muted
            ? "text-lg font-medium text-neutral-400"
            : "text-xl font-semibold text-neutral-900"
        }
      >
        {formatDateHeader(date)}
      </h2>
      {isToday && (
        <span className="rounded-full bg-neutral-900 px-2 py-0.5 text-xs font-medium text-white">
          Today
        </span>
      )}
    </div>
  );
}
