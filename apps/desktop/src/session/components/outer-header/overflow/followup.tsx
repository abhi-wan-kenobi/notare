import { Trans } from "@lingui/react/macro";
import { MailIcon } from "lucide-react";

import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";

import { useFollowupEmail } from "~/services/followup-email";

export function FollowupEmail({
  sessionId,
  onSelect,
}: {
  sessionId: string;
  onSelect?: () => void;
}) {
  const { generateAndDeliver, isGenerating } = useFollowupEmail(sessionId);

  return (
    <DropdownMenuItem
      onClick={() => {
        onSelect?.();
        void generateAndDeliver();
      }}
      disabled={isGenerating}
      className="cursor-pointer"
    >
      <MailIcon />
      <span>
        {isGenerating ? (
          <Trans>Drafting email…</Trans>
        ) : (
          <Trans>Follow-up email</Trans>
        )}
      </span>
    </DropdownMenuItem>
  );
}
