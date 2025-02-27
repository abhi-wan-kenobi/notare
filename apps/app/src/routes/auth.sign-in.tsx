import { zodValidator } from "@tanstack/zod-adapter";
import { createFileRoute, LinkProps } from "@tanstack/react-router";

import { SignedOut, useSignIn } from "@clerk/clerk-react";
import type { OAuthStrategy } from "@clerk/types";

import { schema as connectSchema } from "./auth.connect";
import { createURL } from "../utils";

export const Route = createFileRoute("/auth/sign-in")({
  validateSearch: zodValidator(connectSchema.optional().catch(undefined)),
  component: Component,
});

function Component() {
  const { signIn } = useSignIn();

  if (!signIn) {
    return null;
  }

  const search = Route.useSearch();

  const redirectUrl = createURL("/auth/sso-callback", search).toString();

  const redirectUrlComplete = search
    ? createURL("/auth/connect", search).toString()
    : ("/" satisfies LinkProps["to"]);

  // https://clerk.com/docs/custom-flows/oauth-connections#create-the-sign-up-and-sign-in-flow
  const signInWith = async (strategy: OAuthStrategy) => {
    try {
      await signIn.authenticateWithRedirect({
        strategy,
        redirectUrl,
        redirectUrlComplete,
      });
    } catch (err) {
      // https://clerk.com/docs/custom-flows/error-handling
      console.error(err);
    }
  };

  return (
    <div className="flex items-center justify-center min-h-screen">
      <SignedOut>
        <div>
          <button
            className="border rounded-md px-4 py-2 hover:bg-gray-100"
            onClick={() => signInWith("oauth_google")}
          >
            Sign in with <strong>Google</strong>
          </button>
        </div>
      </SignedOut>
    </div>
  );
}
