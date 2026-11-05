import { type ReactNode, useCallback } from "react";

import { cn } from "@hypr/utils";

import { ChatBody } from "./body";
import { ChatContent } from "./content";
import { ChatSession, type ChatSessionRenderProps } from "./session-provider";
import { ChatToolbarControls } from "./toolbar-controls";
import { useSessionTab } from "./use-session-tab";

import { useLanguageModel } from "~/ai/hooks";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import { useChatActions } from "~/chat/store/use-chat-actions";
import { useShell } from "~/contexts/shell";
import { useOwnerUserId } from "~/shared/owner-user";

export function ChatSessionHost({
  children,
}: {
  children: (sessionProps: ChatSessionRenderProps | null) => ReactNode;
}) {
  const { chat } = useShell();
  const { groupId, sessionId } = chat;
  const { currentSessionId } = useSessionTab();
  const ownerUserId = useOwnerUserId();

  if (!ownerUserId) {
    return <>{children(null)}</>;
  }

  return (
    <ChatSession
      sessionId={sessionId}
      chatGroupId={groupId}
      currentSessionId={currentSessionId}
      unstyled
    >
      {children}
    </ChatSession>
  );
}

export function ChatPanelFrame({
  onDraftContentChange,
  sessionProps,
}: {
  onDraftContentChange?: (hasDraftContent: boolean) => void;
  sessionProps: ChatSessionRenderProps | null;
}) {
  const { chat } = useShell();
  const { groupId, setGroupId } = chat;
  const { panelClassName, toolbarSurface } = useChatAppearance();
  const model = useLanguageModel("chat");

  const handleGroupCreated = useCallback(
    (newGroupId: string) => {
      setGroupId(newGroupId);
    },
    [setGroupId],
  );

  const { handleSendMessage } = useChatActions({
    groupId,
    onGroupCreated: handleGroupCreated,
  });

  return (
    <div
      className={cn([
        "flex h-full min-h-0 flex-col overflow-hidden",
        panelClassName,
      ])}
    >
      <div className="flex h-12 shrink-0 items-center pr-0 pl-0">
        <ChatToolbarControls
          currentChatGroupId={groupId}
          onClose={() => chat.sendEvent({ type: "CLOSE" })}
          onNewChat={chat.startNewChat}
          onSelectChat={chat.selectChat}
          surface={toolbarSurface}
        />
      </div>
      {sessionProps && (
        <ChatContent
          {...sessionProps}
          onDraftContentChange={onDraftContentChange}
          model={model}
          handleSendMessage={handleSendMessage}
        >
          <ChatBody
            messages={sessionProps.messages}
            status={sessionProps.status}
            error={sessionProps.error}
            onReload={sessionProps.regenerate}
            isModelConfigured={!!model}
            hasContext={sessionProps.contextEntities.length > 0}
            onSendMessage={(content, parts) => {
              handleSendMessage(
                content,
                parts,
                sessionProps.sendMessage,
                sessionProps.pendingRefs,
              );
            }}
          />
        </ChatContent>
      )}
    </div>
  );
}
