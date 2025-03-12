import * as React from "react";
import { cn } from "@hypr/ui/lib/utils";

interface BottomSheetProps {
  open: boolean;
  onClose: () => void;
  children: React.ReactNode;
  className?: string;
  preventClose?: boolean;
}

export function BottomSheet({
  open,
  onClose,
  children,
  className,
  preventClose = false,
}: BottomSheetProps) {
  React.useEffect(() => {
    const handleEscapeKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && open && !preventClose) {
        event.preventDefault();
        onClose();
      }
    };

    if (open) {
      document.addEventListener("keydown", handleEscapeKey, true);
    }

    return () => {
      document.removeEventListener("keydown", handleEscapeKey, true);
    };
  }, [open, preventClose, onClose]);

  if (!open) return null;

  return (
    <>
      <div
        className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm"
        aria-hidden="true"
        onClick={preventClose ? undefined : onClose}
      />
      <div
        role="dialog"
        aria-modal="true"
        className={cn(
          "fixed bottom-0 left-0 right-0 z-50",
          "rounded-t-lg bg-neutral-800 shadow-lg",
          "animate-in slide-in-from-bottom duration-300",
          className
        )}
      >
        <div className="flex justify-center pt-2 pb-1">
          <div className="h-1 w-10 rounded-full bg-neutral-600" />
        </div>
        {children}
      </div>
    </>
  );
}

interface BottomSheetContentProps {
  children: React.ReactNode;
  className?: string;
}

export function BottomSheetContent({ 
  children, 
  className 
}: BottomSheetContentProps) {
  return (
    <div className={cn("p-4", className)}>
      {children}
    </div>
  );
}

interface BottomSheetTriggerProps {
  children: React.ReactNode;
  onClick: () => void;
  className?: string;
}

export function BottomSheetTrigger({ 
  children, 
  onClick,
  className 
}: BottomSheetTriggerProps) {
  return (
    <div 
      onClick={onClick}
      className={cn("cursor-pointer", className)}
    >
      {children}
    </div>
  );
}
