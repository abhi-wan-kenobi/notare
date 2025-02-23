import { type ChangeEvent } from "react";
import clsx from "clsx";

interface TitleInputProps {
  value: string;
  onChange: (e: ChangeEvent<HTMLInputElement>) => void;
  onNavigateToEditor?: () => void;
}

export default function TitleInput({ value, onChange, onNavigateToEditor }: TitleInputProps) {
  return (
    <input
      type="text"
      onChange={onChange}
      value={value}
      placeholder="Untitled meeting"
      className={clsx([
        "w-full border-none bg-transparent text-2xl font-bold",
        "focus:outline-none",
      ])}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === "ArrowDown") {
          e.preventDefault();
          onNavigateToEditor?.();
        }
      }}
    />
  );
}
