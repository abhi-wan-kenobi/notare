import { Trans } from "@lingui/react/macro";
import { AlertTriangle } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";

import { useSetSettingValue } from "~/settings/queries";

export function SubscriptionDisclosureModal({
  open,
  onOpenChange,
  onAccept,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAccept: () => void;
}) {
  const setAccepted = useSetSettingValue(
    "llm_subscription_disclosure_accepted",
  );

  const handleAccept = () => {
    setAccepted(true);
    onOpenChange(false);
    onAccept();
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader className="gap-2">
          <div className="flex items-center gap-2 text-amber-500">
            <AlertTriangle className="size-5 shrink-0" />
            <DialogTitle className="text-base font-semibold">
              <Trans>Use Subscription Credentials (Unofficial)</Trans>
            </DialogTitle>
          </div>
          <DialogDescription className="text-muted-foreground flex flex-col gap-2.5 pt-2 text-left text-xs leading-relaxed">
            <span>
              <Trans>
                This feature signs you in using OAuth client credentials
                intended for the vendor's own command-line tools (such as Claude
                Code, Codex, or Gemini CLI).
              </Trans>
            </span>
            <span>
              <Trans>
                <strong>Not endorsed:</strong> This integration is unofficial
                and is not endorsed, certified, or supported by Anthropic,
                OpenAI, or Google.
              </Trans>
            </span>
            <span>
              <Trans>
                <strong>Account risk:</strong> Vendors may modify or disable CLI
                access at any time, and using these tokens in third-party
                software carries the risk of affecting your account standing
                according to the vendor's terms.
              </Trans>
            </span>
            <span>
              <Trans>
                <strong>Privacy:</strong> Prompts, transcripts, and model
                interactions are transmitted directly to the vendor's cloud
                endpoints, identical to standard API usage.
              </Trans>
            </span>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="mt-4 flex flex-row justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            <Trans>Cancel</Trans>
          </Button>
          <Button variant="default" size="sm" onClick={handleAccept}>
            <Trans>I Understand and Accept</Trans>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
