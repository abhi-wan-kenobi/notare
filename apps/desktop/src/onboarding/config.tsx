import { platform } from "@tauri-apps/plugin-os";

import type { SectionStatus } from "./shared";

export type OnboardingStep =
  | "permissions"
  | "login"
  | "calendar"
  | "transcription"
  | "folder-location"
  | "final";

// No account system in Notare: the login step is gone everywhere. The
// calendar step is macOS-only (Apple calendar is local; Google/Outlook
// needed the upstream cloud). folder-location (choose where Markdown notes
// live, e.g. an Obsidian vault) was unreachable upstream — core to Notare,
// so it's in every flow. transcription (download a local STT model) is also
// in every flow — there is no cloud fallback.
const STEPS_MACOS: OnboardingStep[] = [
  "permissions",
  "calendar",
  "transcription",
  "folder-location",
  "final",
];
const STEPS_OTHER: OnboardingStep[] = [
  "transcription",
  "folder-location",
  "final",
];

function getOnboardingSteps(): OnboardingStep[] {
  return platform() === "macos" ? STEPS_MACOS : STEPS_OTHER;
}

export function getInitialStep(): OnboardingStep {
  return getOnboardingSteps()[0];
}

export function getNextStep(
  currentStep: OnboardingStep,
): OnboardingStep | null {
  const steps = getOnboardingSteps();
  const idx = steps.indexOf(currentStep);
  return idx < steps.length - 1 ? steps[idx + 1] : null;
}

export function getPrevStep(
  currentStep: OnboardingStep,
): OnboardingStep | null {
  const steps = getOnboardingSteps();
  const idx = steps.indexOf(currentStep);
  return idx > 0 ? steps[idx - 1] : null;
}

export function getStepStatus(
  step: OnboardingStep,
  currentStep: OnboardingStep,
): SectionStatus | null {
  const steps = getOnboardingSteps();
  const stepIdx = steps.indexOf(step);
  if (stepIdx === -1) return null;
  const currentIdx = steps.indexOf(currentStep);
  if (stepIdx < currentIdx) return "completed";
  if (stepIdx === currentIdx) return "active";
  return "upcoming";
}
