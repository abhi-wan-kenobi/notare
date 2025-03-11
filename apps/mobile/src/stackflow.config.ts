import { defineConfig } from "@stackflow/config";
import { homeActivityLoader } from "./views/home";
import { loginActivityLoader } from "./views/login";
import { onboardingActivityLoader } from "./views/onboarding";
import { noteActivityLoader } from "./views/note";

export const config = defineConfig({
  transitionDuration: 250,
  activities: [
    {
      name: "HomeActivity",
      loader: homeActivityLoader,
    },
    {
      name: "NoteActivity",
      loader: noteActivityLoader,
    },
    {
      name: "LoginActivity",
      loader: loginActivityLoader,
    },
    {
      name: "OnboardingActivity",
      loader: onboardingActivityLoader,
    },
  ],
  initialActivity: () => "LoginActivity",
});
