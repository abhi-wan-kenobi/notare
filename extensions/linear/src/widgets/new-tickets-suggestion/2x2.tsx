import { Button } from "@hypr/ui/components/ui/button";
import {
  WidgetHeader,
  WidgetTwoByTwoWrapper,
  type WidgetTwoByTwo,
} from "@hypr/ui/components/ui/widgets";
import { Maximize2Icon } from "lucide-react";
import TicketCard from "../components/ticket-card";
import { useState } from "react";
import { Ticket } from "../../type";
import { mockNewTickets } from "../../mock";

const NewTicketsSuggestion2x2: WidgetTwoByTwo = ({ onMaximize }) => {
  const [newTickets, _setNewTickets] = useState<Ticket[]>(mockNewTickets);

  return (
    <WidgetTwoByTwoWrapper>
      <div className="p-4 pb-0">
        <WidgetHeader
          title={
            <div className="flex items-center gap-2">
              <img
                src="../assets/linear-icon.png"
                className="size-5 rounded-md"
              />
              New Ticket Suggestion
            </div>
          }
          actions={[
            <Button
              key="maximize"
              variant="ghost"
              size="icon"
              onClick={onMaximize}
              className="p-0"
            >
              <Maximize2Icon size={16} />
            </Button>,
          ]}
        />
      </div>

      <div className="overflow-y-auto flex-1 p-4 pt-0 space-y-3">
        {newTickets.map((ticket) => (
          <TicketCard key={ticket.id} ticket={ticket} />
        ))}
      </div>
    </WidgetTwoByTwoWrapper>
  );
};

export default NewTicketsSuggestion2x2;
