import { DEFAULT_USER_ID } from "~/shared/utils";

// Deterministic ids for humans, session participants, and organizations
// (sync plan 2026-09-07, workstream B4). Two devices that independently mint
// a row for the same email/name/participant converge on one id instead of
// colliding UUIDs. Byte-identical with `crates/db-app/src/id_backfill.rs`;
// the shared test vector lives in `ids.test.ts` and the Rust unit test.
//
// Rules:
// - `norm = lower(trim(value))` after NFC normalization; identity is fixed
//   at creation, so editing an email or name later does not change the id.
// - Empty email (or name) mints no deterministic id: callers keep `id()`.
// - `id = owner_user_id` humans (the self-human) are never rewritten.

const HUMAN_DOMAIN = "notare:human:v1:";
const ORG_DOMAIN = "notare:org:v1:";
const PARTICIPANT_DOMAIN = "notare:participant:v1:";

async function sha256Hex(input: string): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(input),
  );
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

const normalize = (value: string) =>
  value.trim().normalize("NFC").toLowerCase();

/** Deterministic `humans.id` for an email; `null` for an empty email. */
export async function humanIdForEmail(email: string): Promise<string | null> {
  const normalized = normalize(email);
  if (!normalized) return null;
  return `h_${(await sha256Hex(HUMAN_DOMAIN + normalized)).slice(0, 32)}`;
}

/** Deterministic `organizations.id` for a name; `null` for an empty name. */
export async function organizationIdForName(
  name: string,
): Promise<string | null> {
  const normalized = normalize(name);
  if (!normalized) return null;
  return `o_${(await sha256Hex(ORG_DOMAIN + normalized)).slice(0, 32)}`;
}

/**
 * Deterministic `session_participants.id` for a (session, member) pair.
 * `memberKey` is `"h:" + humanId` when a human id is given, else
 * `"e:" + normalized email`; rows with neither keep a random UUID.
 */
export async function participantId(
  sessionId: string,
  humanId: string,
  email: string,
): Promise<string | null> {
  let memberKey: string;
  if (humanId.trim()) {
    memberKey = `h:${humanId}`;
  } else {
    const normalized = normalize(email);
    if (!normalized) return null;
    memberKey = `e:${normalized}`;
  }
  const digest = await sha256Hex(
    `${PARTICIPANT_DOMAIN}${sessionId}\n${memberKey}`,
  );
  return `sp_${digest.slice(0, 32)}`;
}

/**
 * Convenience for the self-participant row inserted by `createSession`:
 * never a UUID — the pair (session, DEFAULT_USER_ID human) is always
 * deterministic.
 */
export async function selfParticipantId(
  sessionId: string,
  userId: string = DEFAULT_USER_ID,
): Promise<string> {
  return (await participantId(sessionId, userId, ""))!;
}
