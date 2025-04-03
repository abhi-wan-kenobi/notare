import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { Session } from "@hypr/plugin-db";
import { OngoingSessionProvider } from "@hypr/utils/contexts";
import { SessionsProvider } from "@hypr/utils/contexts";
import { createSessionsStore, createSessionStore } from "@hypr/utils/stores";

const queryClient = new QueryClient();

export default function MockProvider({
  children,
  sessionId,
}: {
  children: React.ReactNode;
  sessionId?: string;
}) {
  const sessionsStore = createSessionsStore();

  const session: Session = {
    id: sessionId ?? crypto.randomUUID(),
    created_at: new Date().toISOString(),
    visited_at: new Date().toISOString(),
    user_id: "1",
    calendar_event_id: "1",
    title: "Test Session",
    raw_memo_html: "Test Transcript",
    enhanced_memo_html: null,
    conversations: [],
  };
  sessionsStore.setState({
    currentSessionId: session.id,
    sessions: {
      [session.id]: createSessionStore(session),
    },
  });

  return (
    <QueryClientProvider client={queryClient}>
      <OngoingSessionProvider>
        <SessionsProvider store={sessionsStore}>
          {children}
        </SessionsProvider>
      </OngoingSessionProvider>
    </QueryClientProvider>
  );
}
