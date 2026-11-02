export interface AttendeeInput {
  email?: string | null;
  name?: string | null;
  isCurrentUser?: boolean | null;
}

const PUBLIC_EMAIL_DOMAINS: Record<string, true> = {
  "gmail.com": true,
  "googlemail.com": true,
  "yahoo.com": true,
  "hotmail.com": true,
  "outlook.com": true,
  "icloud.com": true,
  "me.com": true,
  "aol.com": true,
  "proton.me": true,
  "protonmail.com": true,
};

/**
 * Extracts domain from an email address in lowercase.
 */
export function getEmailDomain(email: string): string | null {
  const atIdx = email.lastIndexOf("@");
  if (atIdx === -1 || atIdx === email.length - 1) return null;
  return email
    .slice(atIdx + 1)
    .toLowerCase()
    .trim();
}

/**
 * Filter external attendees from an attendee list.
 *
 * Rules:
 * 1. Exclude the current user (via isCurrentUser flag or matching current user email).
 * 2. Exclude internal attendees matching the organization domain (unless user is on a generic public domain).
 * 3. Filter out invalid/empty emails.
 * 4. Deduplicate case-insensitively.
 * 5. Cap at 10 recipients maximum.
 */
export function extractExternalAttendees(
  attendees: Array<AttendeeInput | string>,
  options?: {
    currentUserEmail?: string | null;
    internalDomain?: string | null;
  },
): string[] {
  const currentUserEmail =
    options?.currentUserEmail?.trim().toLowerCase() ?? null;
  let internalDomain = options?.internalDomain?.trim().toLowerCase() ?? null;

  if (!internalDomain && currentUserEmail) {
    const domain = getEmailDomain(currentUserEmail);
    if (domain && !PUBLIC_EMAIL_DOMAINS[domain]) {
      internalDomain = domain;
    }
  }

  const seen = new Set<string>();
  const external: string[] = [];

  for (const item of attendees) {
    const email = (typeof item === "string" ? item : item.email)
      ?.trim()
      .toLowerCase();
    const isCurrentUser =
      typeof item === "object" ? item?.isCurrentUser : false;

    if (!email || !email.includes("@")) continue;
    if (isCurrentUser) continue;
    if (currentUserEmail && email === currentUserEmail) continue;

    const domain = getEmailDomain(email);
    if (internalDomain && domain === internalDomain) continue;

    if (!seen.has(email)) {
      seen.add(email);
      external.push(email);
      if (external.length === 10) break;
    }
  }

  return external;
}
