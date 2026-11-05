import { ChevronDown } from "lucide-react";
import type { ReactNode } from "react";

import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

export function SectionLabel({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <h2
      className={cn([
        "text-muted-foreground border-border/70 border-b pb-2 text-[10px] font-semibold tracking-[0.09em] uppercase",
        className,
      ])}
    >
      {children}
    </h2>
  );
}

export function SettingsRow({
  title,
  description,
  trailing,
  children,
  className,
}: {
  title: ReactNode;
  description?: ReactNode;
  trailing?: ReactNode;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn([
        "border-border/60 flex min-h-12 items-center justify-between gap-4 border-b py-3 last:border-b-0",
        className,
      ])}
    >
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium">{title}</div>
        {description != null && (
          <div className="text-muted-foreground mt-0.5 text-xs">
            {description}
          </div>
        )}
        {children != null && <div className="mt-3">{children}</div>}
      </div>
      {trailing != null && <div className="shrink-0">{trailing}</div>}
    </div>
  );
}

export function SettingsToggleRow({
  title,
  description,
  checked,
  onChange,
  disabled,
}: {
  title: ReactNode;
  description?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <SettingsRow
      title={title}
      description={description}
      trailing={
        <Switch
          checked={checked}
          onCheckedChange={onChange}
          disabled={disabled}
          aria-label={typeof title === "string" ? title : undefined}
        />
      }
    />
  );
}

export function SettingsExpander({
  title,
  description,
  children,
  defaultOpen = false,
  trailing,
  className,
}: {
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  trailing?: ReactNode;
  className?: string;
}) {
  return (
    <details
      className={cn([
        "group border-border/60 border-b py-3 last:border-b-0",
        className,
      ])}
      open={defaultOpen || undefined}
    >
      <summary className="flex cursor-pointer list-none items-center justify-between gap-4 [&::-webkit-details-marker]:hidden">
        <span className="min-w-0">
          <span className="block text-sm font-medium">{title}</span>
          {description != null && (
            <span className="text-muted-foreground mt-0.5 block text-xs">
              {description}
            </span>
          )}
        </span>
        <div className="flex shrink-0 items-center gap-2">
          {trailing}
          <ChevronDown
            className="text-muted-foreground size-4 transition-transform duration-200 group-open:rotate-180"
            aria-hidden="true"
          />
        </div>
      </summary>
      <div className="pt-4">{children}</div>
    </details>
  );
}
