/**
 * Marker-delimited `## Action Items` markdown section (WS-C write-back / WS-D2).
 *
 * SQLite is authoritative: this section is a *rendered projection* of the
 * action_items rows. It is written into a note's markdown (editor note and the
 * fs-sync `_memo.md`) between HTML-comment markers so it can be regenerated
 * IN PLACE without touching anything the user wrote around it — the key
 * requirement for surviving Obsidian round-trips.
 *
 * Regeneration semantics:
 *   - `upsertActionItemsSection` replaces ONLY the marked region (or appends it
 *     when absent); every other byte of the note is preserved verbatim.
 *   - Item text/owner/due come from SQLite on every regen (SQLite wins). A
 *     user's edits *inside* the region are overwritten — that's intentional;
 *     inbound edits are checkbox toggles only (status), handled upstream.
 *   - Empty item set removes the section entirely (no orphan heading).
 *
 * The markers are HTML comments: invisible in rendered markdown and preserved
 * by the prosemirror<->markdown round-trip.
 */

export const SECTION_START = "<!-- notare:action-items -->";
export const SECTION_END = "<!-- /notare:action-items -->";
const HEADING = "## Action Items";

export type ActionItemLine = {
  text: string;
  done: boolean;
  /** ISO date (YYYY-MM-DD) or "". */
  dueAt?: string;
  /** Human-readable owner label (already resolved) or "". */
  ownerLabel?: string;
};

/** Render a single checklist line: `- [ ] text 📅 YYYY-MM-DD @owner`. */
function renderLine(item: ActionItemLine): string {
  const box = item.done ? "[x]" : "[ ]";
  const parts = [`- ${box} ${item.text.trim()}`];
  if (item.dueAt) parts.push(`📅 ${item.dueAt}`);
  if (item.ownerLabel)
    parts.push(`@${item.ownerLabel.trim().replace(/\s+/g, "-")}`);
  return parts.join(" ");
}

/**
 * The rendered section body, INCLUDING the start/end markers. Returns "" when
 * there are no items (caller removes the region entirely).
 */
export function renderActionItemsSection(items: ActionItemLine[]): string {
  if (items.length === 0) return "";
  const lines = items.map(renderLine);
  return [SECTION_START, HEADING, "", ...lines, SECTION_END].join("\n");
}

/**
 * Insert/replace/remove the action-items section in `markdown`, preserving all
 * surrounding content. Idempotent: upserting the same items twice yields the
 * same document.
 */
export function upsertActionItemsSection(
  markdown: string,
  items: ActionItemLine[],
): string {
  const source = markdown ?? "";
  const startIdx = source.indexOf(SECTION_START);
  const rendered = renderActionItemsSection(items);

  if (startIdx === -1) {
    // No existing section.
    if (!rendered) return source; // nothing to add
    const base = source.replace(/\s+$/, "");
    return base.length > 0 ? `${base}\n\n${rendered}\n` : `${rendered}\n`;
  }

  // Existing section: find its end (end marker after start).
  const endMarkerIdx = source.indexOf(SECTION_END, startIdx);
  const regionEnd =
    endMarkerIdx === -1 ? source.length : endMarkerIdx + SECTION_END.length;

  const before = source.slice(0, startIdx);
  const after = source.slice(regionEnd);

  if (!rendered) {
    // Remove the section and collapse the blank lines it leaves behind.
    const merged = `${before.replace(/\s+$/, "")}\n${after.replace(/^\s+/, "")}`;
    return (
      merged
        .replace(/\n{3,}/g, "\n\n")
        .replace(/^\n+/, "")
        .replace(/\s+$/, "") + "\n"
    );
  }

  const beforeTrimmed = before.replace(/\s+$/, "");
  const afterTrimmed = after.replace(/^\s+/, "");
  const head = beforeTrimmed.length > 0 ? `${beforeTrimmed}\n\n` : "";
  const tail = afterTrimmed.length > 0 ? `\n\n${afterTrimmed}` : "";
  return `${head}${rendered}${tail}\n`.replace(/^\n+/, "");
}

/** Whether the note currently contains a rendered action-items section. */
export function hasActionItemsSection(markdown: string): boolean {
  return (markdown ?? "").includes(SECTION_START);
}

/** A checkbox state parsed back out of a rendered action-items line. */
export type ParsedActionItemLine = {
  /** The item text with the trailing `📅 date` / `@owner` chips stripped. */
  text: string;
  done: boolean;
};

/**
 * Strip the trailing render chips (`📅 YYYY-MM-DD` then `@owner`, appended in
 * that order by `renderLine`) to recover the identity text. Chips are removed
 * from the end so text that itself contains an `@` earlier in the line is
 * preserved.
 *
 * KNOWN LIMITATION (inbound is checkbox-toggle only): we recover identity by
 * text, so a user editing the *text* of a line on disk will simply fail to
 * match a SQLite row (the toggle is ignored) — text edits are NOT synced back.
 */
function stripRenderChips(rest: string): string {
  let text = rest;
  // Trailing `@owner` chip (owner labels are hyphen-joined, no whitespace).
  text = text.replace(/\s+@\S+\s*$/u, "");
  // Trailing `📅 YYYY-MM-DD` chip.
  text = text.replace(/\s+📅\s+\S+\s*$/u, "");
  return text.trim();
}

/**
 * Parse the checkbox lines inside the marked action-items region back into
 * `{ text, done }` states. Lines outside the markers are ignored. This is the
 * inbound half of the round-trip: it lets an external `- [ ]`↔`- [x]` edit in
 * the memo be reconciled against SQLite (status only — see `stripRenderChips`).
 */
export function parseActionItemsSection(
  markdown: string,
): ParsedActionItemLine[] {
  const source = markdown ?? "";
  const startIdx = source.indexOf(SECTION_START);
  if (startIdx === -1) return [];
  const endMarkerIdx = source.indexOf(SECTION_END, startIdx);
  const regionEnd = endMarkerIdx === -1 ? source.length : endMarkerIdx;
  const region = source.slice(startIdx + SECTION_START.length, regionEnd);

  const results: ParsedActionItemLine[] = [];
  for (const line of region.split("\n")) {
    const match = /^\s*-\s+\[([ xX])\]\s+(.*)$/u.exec(line);
    if (!match) continue;
    const done = match[1]!.toLowerCase() === "x";
    const text = stripRenderChips(match[2]!);
    if (text) results.push({ text, done });
  }
  return results;
}

/** A subset of an action_items row needed to render a line. */
export type ActionItemRow = {
  text: string;
  status: string;
  due_at?: string | null;
  owner_speaker_id?: string | null;
};

/**
 * Adapt SQLite action_items rows to render lines + upsert them into a note.
 * `resolveOwnerLabel` maps an owner_speaker_id to a display label (or "" to
 * omit the owner chip). Pure — this is the whole DB→markdown bridge, so the
 * caller only supplies the current note markdown, the rows, and the resolver.
 */
export function writeActionItemsToMarkdown(
  noteMarkdown: string,
  rows: ActionItemRow[],
  resolveOwnerLabel: (ownerSpeakerId: string) => string = () => "",
): string {
  const lines: ActionItemLine[] = rows.map((row) => ({
    text: row.text,
    done: row.status === "done" || row.status === "completed",
    dueAt: (row.due_at ?? "").trim() || undefined,
    ownerLabel: row.owner_speaker_id
      ? resolveOwnerLabel(row.owner_speaker_id).trim() || undefined
      : undefined,
  }));
  return upsertActionItemsSection(noteMarkdown, lines);
}
