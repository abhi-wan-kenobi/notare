import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import { useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as sfxCommands } from "@hypr/plugin-sfx";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { OnboardingButton } from "./shared";
import {
  getOrCreateWelcomeSession,
  setPendingWelcomeSession,
} from "./welcome-note";

import { flushAutomaticRelaunch } from "~/shared/relaunch";
import { commands } from "~/types/tauri.gen";

const SOCIALS = [
  {
    label: "GitHub",
    icon: "simple-icons:github",
    url: "https://github.com/abhi-wan-kenobi/notare",
  },
] as const;

const SOCIAL_ICON_SIZE = 18;

export function FinalDescription() {
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
      <span>
        <Trans>Join our community and stay updated:</Trans>
      </span>
      <div className="flex items-center gap-2">
        {SOCIALS.map((social) => {
          return (
            <button
              key={social.label}
              onClick={() => void openerCommands.openUrl(social.url, null)}
              className="text-muted-foreground hover:text-muted-foreground inline-flex size-5 items-center justify-center rounded-md transition-colors duration-150"
              aria-label={social.label}
            >
              <Icon
                icon={social.icon}
                width={SOCIAL_ICON_SIZE}
                height={SOCIAL_ICON_SIZE}
              />
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function FinalSection({
  onContinue,
}: {
  onContinue: (sessionId: string) => void;
}) {
  const { t } = useLingui();
  const [pending, setPending] = useState(false);

  const handleClick = async () => {
    setPending(true);
    try {
      await finishOnboarding(onContinue);
    } catch (error) {
      console.error("[onboarding] failed to finish", error);
      sonnerToast.error(t`Couldn't open Notare. Please try again.`);
    } finally {
      setPending(false);
    }
  };

  return (
    <OnboardingButton
      className="px-6 py-2 text-sm disabled:opacity-50"
      disabled={pending}
      onClick={() => void handleClick()}
    >
      <Trans>Open Notare</Trans>
    </OnboardingButton>
  );
}

export async function finishOnboarding(
  onContinue?: (sessionId: string) => void,
) {
  await sfxCommands.stop("BGM").catch(console.error);
  const welcomeSessionId = await getOrCreateWelcomeSession();
  await new Promise((resolve) => setTimeout(resolve, 100));
  await commands.setOnboardingNeeded(false).catch(console.error);
  await new Promise((resolve) => setTimeout(resolve, 100));
  await analyticsCommands
    .event({ event: "onboarding_completed" })
    .catch(console.error);
  setPendingWelcomeSession(welcomeSessionId);
  // A failed relaunch must not strand the user on this screen; the relaunch
  // stays pending and the app is usable in the meantime.
  const relaunching = await flushAutomaticRelaunch().catch((error) => {
    console.error("[onboarding] automatic relaunch failed", error);
    return false;
  });
  if (relaunching) {
    return;
  }
  setPendingWelcomeSession(null);
  onContinue?.(welcomeSessionId);
}
