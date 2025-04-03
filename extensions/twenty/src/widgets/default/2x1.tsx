import { WidgetTwoByOne, WidgetTwoByOneWrapper } from "@hypr/ui/components/ui/widgets";

import { commands as dbCommands } from "@hypr/plugin-db";
import { useSessions } from "@hypr/utils/contexts";
import { useQuery } from "@tanstack/react-query";

import * as twenty from "../../client";

const Twenty2x1: WidgetTwoByOne = () => {
  const sessionId = useSessions((s) => s.currentSessionId);

  return (
    <WidgetTwoByOneWrapper>
      {sessionId && <Inner sessionId={sessionId} />}
    </WidgetTwoByOneWrapper>
  );
};

function Inner({ sessionId }: { sessionId: string }) {
  const participants = useQuery({
    queryKey: ["session-participants", sessionId],
    queryFn: () => dbCommands.sessionListParticipants(sessionId),
  });

  const twentyHumans = useQuery({
    enabled: !!participants.data,
    queryKey: ["twenty-humans", sessionId],
    queryFn: () => twenty.findManyPeople("TODO"),
  });

  return <pre>{JSON.stringify(twentyHumans.data, null, 2)}</pre>;
}

export default Twenty2x1;
