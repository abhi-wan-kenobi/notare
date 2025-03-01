import type { Extension } from "@hypr/extension-utils";

import NewTicketsSuggestion from "./widgets/new-tickets-suggestion";
import RelatedTickets from "./widgets/related-tickets";

const extension: Extension = {
  newTicketsSuggestion: NewTicketsSuggestion,
  relatedTickets: RelatedTickets,
};

export default extension;
