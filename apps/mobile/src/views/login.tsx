import { AppScreen } from "@stackflow/plugin-basic-ui";
import { type ActivityComponentType } from "@stackflow/react/future";

import { Particles } from "@hypr/ui/components/ui/particles";
import PushableButton from "@hypr/ui/components/ui/pushable-button";
import { TextAnimate } from "@hypr/ui/components/ui/text-animate";

export const LoginActivity: ActivityComponentType<"LoginActivity"> = () => {
  const handleSignIn = () => {
    // TODO: Need to redirect to https://app.hyprnote.com/auth/connect and wait for a callback
  };

  return (
    <AppScreen>
      <div className="relative p-6 flex h-full flex-col pt-12">
        <main className="z-10 flex w-full flex-col items-center justify-between flex-1">
          <div className="flex flex-col items-center justify-center h-full">
            <TextAnimate
              animation="blurIn"
              as="h1"
              once
              className="mb-4 font-racing-sans text-5xl font-bold"
            >
              HYPRNOTE
            </TextAnimate>

            <TextAnimate
              animation="slideUp"
              by="word"
              once
              className="mb-12 text-center text-lg font-medium text-neutral-600 md:text-xl lg:text-2xl"
            >
              AI notepad for meetings
            </TextAnimate>
          </div>

          <PushableButton onClick={handleSignIn} className="mb-4 w-full">
            Get Started
          </PushableButton>
        </main>

        <Particles
          className="absolute inset-0 z-0"
          quantity={100}
          ease={80}
          color={"#000000"}
          refresh
        />
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    LoginActivity: {};
  }
}
