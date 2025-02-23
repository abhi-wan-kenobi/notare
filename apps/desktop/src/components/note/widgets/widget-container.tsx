import { type ReactNode } from "react";
import { type WidgetSize } from "./types";
import clsx from "clsx";

interface WidgetContainerProps {
  size: WidgetSize;
  children: ReactNode;
}

const sizeClasses = {
  small: "w-[160px] h-[160px]",
  medium: "w-[340px] h-[160px]",
  large: "w-[340px] h-[340px]",
};

export function WidgetContainer({ size, children }: WidgetContainerProps) {
  return (
    <div
      className={clsx(
        "rounded-lg border bg-white p-4",
        sizeClasses[size]
      )}
    >
      {children}
    </div>
  );
}
