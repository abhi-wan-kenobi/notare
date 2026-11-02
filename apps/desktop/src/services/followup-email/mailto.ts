/**
 * Builds a valid `mailto:` URL with properly encoded recipients, subject, and body.
 */
export function buildMailtoUrl(
  to: string[],
  subject: string,
  body: string,
): string {
  const recipients = to
    .filter(Boolean)
    .map((e) => e.trim())
    .join(",");
  const params: string[] = [];

  if (subject) {
    params.push(`subject=${encodeURIComponent(subject)}`);
  }
  if (body) {
    params.push(`body=${encodeURIComponent(body)}`);
  }

  const query = params.length > 0 ? `?${params.join("&")}` : "";
  return `mailto:${recipients}${query}`;
}
