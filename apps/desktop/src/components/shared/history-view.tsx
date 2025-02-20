interface ChatItem {
  id: string;
  title: string;
  time: string;
}

interface HistoryViewProps {
  chatHistory: ChatItem[];
  onChatSelect: () => void;
}

export default function HistoryView({
  chatHistory,
  onChatSelect,
}: HistoryViewProps) {
  return (
    <div className="flex-1 overflow-y-auto p-4">
      <div className="mb-6">
        <input
          type="text"
          placeholder="Search or start new chat"
          className="w-full rounded-lg border bg-neutral-50 px-3 py-2 text-sm placeholder:text-neutral-500 focus:border-blue-500 focus:outline-none"
        />
      </div>
      <div className="space-y-1">
        <div className="mb-3 text-xs font-medium text-neutral-500">Today</div>
        {chatHistory.slice(0, 3).map((chat) => (
          <ChatHistoryItem key={chat.id} chat={chat} onClick={onChatSelect} />
        ))}
        <div className="mb-3 mt-6 text-xs font-medium text-neutral-500">
          Older
        </div>
        {chatHistory.slice(3).map((chat) => (
          <ChatHistoryItem key={chat.id} chat={chat} onClick={onChatSelect} />
        ))}
      </div>
    </div>
  );
}

function ChatHistoryItem({
  chat,
  onClick,
}: {
  chat: ChatItem;
  onClick: () => void;
}) {
  return (
    <button
      className="flex w-full items-center gap-3 rounded-lg p-2 hover:bg-neutral-100"
      onClick={onClick}
    >
      <div className="flex-1 truncate text-left">
        <div className="text-sm">{chat.title}</div>
        <div className="text-xs text-neutral-500">{chat.time}</div>
      </div>
    </button>
  );
}
