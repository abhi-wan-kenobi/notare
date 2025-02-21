import { Button } from "@hypr/ui/components/hypr-ui/button";
import { ChevronLeft, HistoryIcon, PlusCircleIcon, XIcon } from "lucide-react";

interface HeaderProps {
  title: string;
  showHistory: boolean;
  onBackClick: () => void;
  onNewChat: () => void;
  onHistoryClick: () => void;
  onCloseClick: () => void;
}

export default function Header({
  title,
  showHistory,
  onBackClick,
  onNewChat,
  onHistoryClick,
  onCloseClick,
}: HeaderProps) {
  if (showHistory) {
    return (
      <div className="flex items-center justify-between px-2 py-2">
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            className="rounded-lg p-1"
            onClick={onBackClick}
          >
            <ChevronLeft size={20} />
          </Button>
          <div className="text-sm font-medium">All chats</div>
        </div>

        <Button variant="ghost" size="sm" onClick={onNewChat}>
          New chat
        </Button>
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between px-4 py-2 pr-2">
      <div className="text-sm font-medium">{title}</div>
      <div className="flex items-center gap-2">
        <Button variant="ghost" className="rounded-lg p-1" onClick={onNewChat}>
          <PlusCircleIcon className="size-4" />
        </Button>

        <Button
          variant="ghost"
          className="rounded-lg p-1"
          onClick={onHistoryClick}
        >
          <HistoryIcon className="size-4" />
        </Button>

        <Button
          variant="ghost"
          className="rounded-lg p-1"
          onClick={onCloseClick}
        >
          <XIcon className="size-4" />
        </Button>
      </div>
    </div>
  );
}
