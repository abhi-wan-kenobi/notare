# Connect Gmail to Notare (Beta)

Notare offers an opt-in Gmail integration specifically for creating **follow-up email drafts** after meetings.

Like Google Calendar, Notare communicates directly with the Google APIs from your computer via a local loopback (`127.0.0.1`). There is no intermediate cloud server, no proxy, and your emails never leave your machine.

---

## Key Principles & Privacy Posture

1. **Separate from Calendar**: The Gmail integration is completely distinct from the Google Calendar integration. Connecting or disconnecting one has no effect on the other.
2. **Draft creation only (`gmail.compose`)**: Notare requests only permission to compose and draft emails (`https://www.googleapis.com/auth/gmail.compose`). It **cannot read** your inbox, cannot read received messages, and **never auto-sends** emails.
3. **No automatic sending**: Every follow-up email is created as a draft in your Gmail account. You review, edit, and send it yourself from Gmail.
4. **Local token storage**: Refresh tokens are stored securely in your operating system's keychain under `google-mail/oauth/<account_email>`. Access tokens exist in memory only and expire automatically.

---

## Build Modes & Fallbacks

- **Official builds**: Baked with a compile-time Gmail client (`NOTARE_GMAIL_CLIENT_ID` and `NOTARE_GMAIL_CLIENT_SECRET`). In **Settings → Integrations → Gmail (beta)**, you can connect your Google account with one click.
- **Development & source builds**: When the compile-time client ID is absent, Notare automatically falls back to:
  1. Copying the generated follow-up draft body to your clipboard.
  2. Opening your system's default email client via a pre-filled `mailto:` URL with the external recipients and subject line.

---

## Follow-up Email Drafts

After a meeting or on-demand from the note's overflow menu (`...` → **Follow-up email**):

1. **Attendee filtering**: Notare detects meeting participants and filters for external recipients:
   - Excludes your own email.
   - Excludes internal company domain colleagues.
   - Caps at 10 recipients maximum.
2. **AI draft generation**: Uses your configured `notes` model to structure:
   - **Recap**: Key topics discussed.
   - **Decisions**: Formal agreements and outcomes.
   - **Action items**: Concrete tasks with assigned owners.
   - **Next steps**: Milestones and upcoming check-ins.
3. **Delivery**:
   - If Gmail is connected: Creates a draft in your Gmail account via `users.drafts.create`.
   - Otherwise: Copies body to clipboard and opens `mailto:`.

---

## Managing Accounts

In **Settings → Integrations → Gmail (beta)**:
- **Connect Account**: Authenticate a Google account.
- **Disconnect**: Revoke and delete the refresh token for a specific account.
- **Multiple accounts**: Support for connecting multiple accounts and choosing which account receives the draft.
