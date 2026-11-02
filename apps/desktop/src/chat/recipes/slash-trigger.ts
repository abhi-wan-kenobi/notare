export interface SlashTriggerMatch {
  query: string;
  from: number;
  to: number;
}

/**
 * Detects whether the text before the cursor triggers the recipe slash command menu.
 * Matches when '/' is:
 * 1. At the start of the input
 * 2. Preceded by whitespace (space, tab, newline)
 *
 * Does NOT match:
 * 1. Inside a word (e.g. "foo/bar", "and/or")
 * 2. Inside a URL (e.g. "https://example.com/test", "http://localhost/path", "ftp://server/file")
 * 3. When the query contains whitespace (space/newline closes the command trigger)
 */
export function detectSlashTrigger(
  textBefore: string,
  cursorOffset?: number,
): SlashTriggerMatch | null {
  const text =
    cursorOffset !== undefined ? textBefore.slice(0, cursorOffset) : textBefore;

  const slashIndex = text.lastIndexOf("/");
  if (slashIndex === -1) {
    return null;
  }

  // Check 1: Must be at start of input or preceded by whitespace
  if (slashIndex > 0) {
    const charBefore = text[slashIndex - 1];
    if (!/\s/.test(charBefore)) {
      // Preceded by a non-whitespace character (e.g. "foo/bar", "http:/") -> not a trigger
      return null;
    }
  }

  // Check 2: Not inside or part of a URL
  // Check the token immediately before the slash if any
  const lastSpaceIndex = text.lastIndexOf(" ", slashIndex - 1);
  const tokenStart = lastSpaceIndex === -1 ? 0 : lastSpaceIndex + 1;
  const tokenBeforeSlash = text.slice(tokenStart, slashIndex);

  // If the token immediately before the slash is a URL protocol/prefix
  // ("https:" + "/" typed as separate tokens), it is not a trigger. A URL
  // elsewhere in the text, followed by whitespace, does not suppress one.
  if (
    /^(https?|ftp|file):/i.test(tokenBeforeSlash) ||
    /^www\./i.test(tokenBeforeSlash)
  ) {
    return null;
  }

  // Also verify that the slash is not part of a URL pattern
  const precedingChunk = text.slice(Math.max(0, slashIndex - 100), slashIndex);
  if (/https?:\/\/[^\s]+$/i.test(precedingChunk)) {
    return null;
  }

  // Check 3: Query is everything from slash to end (or cursor)
  const query = text.slice(slashIndex + 1);

  // Whitespace in query means the slash trigger is no longer active
  if (/\s/.test(query)) {
    return null;
  }

  return {
    query,
    from: slashIndex,
    to: text.length,
  };
}

/**
 * Filters recipes by matching the query against title and prompt.
 * If query is empty, returns all recipes.
 */
export function filterRecipes<T extends { title: string; prompt?: string }>(
  recipes: T[],
  query: string,
): T[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) {
    return recipes;
  }

  return recipes.filter((recipe) => {
    const titleMatch = recipe.title.toLowerCase().includes(trimmed);
    const promptMatch = recipe.prompt
      ? recipe.prompt.toLowerCase().includes(trimmed)
      : false;
    return titleMatch || promptMatch;
  });
}
