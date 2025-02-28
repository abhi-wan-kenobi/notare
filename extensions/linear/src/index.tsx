import type { Extension } from "@hypr/extension-utils";

import NewTicketsSuggestion2x2 from "./widgets/new-tickets-suggestion/2x2";
import NewTicketsSuggestionFull from "./widgets/new-tickets-suggestion/full";
import { init as initNewTicketsSuggestion } from "./widgets/new-tickets-suggestion/init";

import RelatedTickets2x2 from "./widgets/related-tickets/2x2";
import RelatedTicketsFull from "./widgets/related-tickets/full";
import { init as initRelatedTickets } from "./widgets/related-tickets/init";

const extension: Extension = {
  newTicketsSuggestion: [
    {
      id: "new-tickets-suggestion-2x2",
      init: initNewTicketsSuggestion,
      component: NewTicketsSuggestion2x2,
    },
    {
      id: "new-tickets-suggestion-full",
      init: initNewTicketsSuggestion,
      component: NewTicketsSuggestionFull,
    },
  ],
  relatedTickets: [
    {
      id: "related-tickets-2x2",
      init: initRelatedTickets,
      component: RelatedTickets2x2,
    },
    {
      id: "related-tickets-full",
      init: initRelatedTickets,
      component: RelatedTicketsFull,
    },
  ],
};

export default extension;
