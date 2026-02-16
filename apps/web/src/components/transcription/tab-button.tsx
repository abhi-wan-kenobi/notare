import { cn } from "@hypr/utils";

export function TabButton({
  label,
  active,
  onClick,
  trailing,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
  trailing?: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={cn([
        "flex items-center px-3 py-2.5 text-sm transition-colors relative",
        active
          ? "text-neutral-900 font-medium"
          : "text-neutral-500 hover:text-neutral-700",
      ])}
    >
      {label}
      {trailing}
      {active && (
        <span className="absolute bottom-0 left-3 right-3 h-0.5 bg-neutral-900 rounded-full" />
      )}
    </button>
  );
}
