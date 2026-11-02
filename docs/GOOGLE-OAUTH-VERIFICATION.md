# Google OAuth verification — what to submit and the demo video

Status 2026-11-02: the first-party client (`NOTARE_GOOGLE_CLIENT_ID`, GCP project
"Notare") is in **Testing**. That means ≤100 test users and refresh tokens that die after
**7 days**. Moving the consent screen to **Production** and passing verification removes
both limits. This doc is the checklist and the shot list for the video Google asks for.

Scopes requested (`crates/google-oauth/src/flow.rs:15-17`):

| Scope | Class | Why Notare needs it (use this wording in the form) |
|---|---|---|
| `calendar.readonly` | sensitive | Lists the user's calendars so they can pick which ones to watch |
| `calendar.events.readonly` | sensitive | Reads upcoming events to show "meeting starting now" and attach attendees to a note |
| `userinfo.email` | non-sensitive | Shows which Google account is connected; keys the stored refresh token |

No restricted scopes. **Do not add `gmail.*` to this client** — restricted scopes trigger a
CASA security assessment. Email features use a separate opt-in client (decision 2026-11-02).

## 1. Console checklist (APIs & Services → OAuth consent screen)

- App name: `Notare`. Logo: `docs/logo/` 120×120 PNG. Support email: a @notare.dev or
  personal address you will answer.
- App domain: `https://notare.dev`. Home page `https://notare.dev`, privacy policy
  `https://notare.dev/privacy`, terms `https://notare.dev/terms` (both pages must load
  without auth and mention Google user data handling — see §3).
- Authorized domain: `notare.dev`. Verify ownership in Search Console with the same Google
  account that owns the GCP project.
- Scopes: exactly the three above. Remove anything else left over from testing.
- "Publishing status" → **Publish app** → then **Prepare for verification**.
- Verification form questions and answers:
  - *How will the scopes be used?* — one sentence each from the table.
  - *Does the app run on a server?* — No. Desktop application; OAuth uses the installed-app
    flow with PKCE and a `127.0.0.1` loopback redirect. Tokens are stored in the OS
    keychain on the user's machine. No Notare server receives Google data.
  - *Demo video link* — unlisted YouTube, §2.

Expected turnaround: brand verification a few days; sensitive-scope review 2–6 weeks; they
usually reply with one round of questions (most common: "show the scope in use", "privacy
policy must name Google data").

## 2. Demo video — shot list (target 3–4 minutes, no cuts between shots 3–6)

Google reviewers check four things: the app name matches, the client ID on the consent URL
matches the project, every requested scope is visibly *used*, and the privacy policy is
reachable. Record at 1080p with the browser's URL bar always visible. English narration
or captions; no music.

| # | Shot | What must be on screen | Say |
|---|---|---|---|
| 1 | Notare launched, fresh profile, Settings → Calendar | Title bar "Notare", the **Sign in with Google** button, no BYO JSON step | "Notare is an open-source, local-first meeting notetaker. Calendar access is optional and read-only." |
| 2 | Click **Sign in with Google** | System browser opens `accounts.google.com/o/oauth2/auth?...client_id=<id>...` — pause so the **client_id** in the URL bar is legible; redirect_uri shows `http://127.0.0.1:<port>/oauth/callback` | "The app opens the system browser; the redirect goes back to the app on localhost. Nothing goes through our servers." |
| 3 | Consent screen | App name **Notare**, the three scopes listed with Google's wording, the privacy-policy link visible | "These are the only scopes we request." (If the unverified warning appears, click Advanced → Go to Notare, say "this warning goes away after verification.") |
| 4 | Allow → back in Notare | Calendar settings switch to the connected state (calendar toggles + **Disconnect**); if the account email is not displayed, say it aloud (`userinfo.email` labels the keychain entry) | "The email is used only to label the connection." |
| 5 | Calendar list | The user's calendars appear with toggles (`calendar.readonly`) | "We list calendars so the user chooses which to watch." |
| 6 | Events | Today's events in the sidebar / "starting now" banner with attendees (`calendar.events.readonly`); open one event → new note pre-filled with title + attendees | "Event titles and attendees prefill a note. Notare never writes to the calendar." |
| 7 | Disconnect | Settings → Disconnect; show the account gone | "Users can revoke at any time; the token is deleted from the OS keychain." |
| 8 | Privacy policy | Browser at `https://notare.dev/privacy`, scroll to the Google data section | "Our policy states that Google data stays on the user's device." |

Preparation before recording:
- Use a Google test account with 2 calendars and 3+ events today (one with attendees).
- Build with the bundled client (any `release.yaml` build after 2026-11-02 or a local build
  with `NOTARE_GOOGLE_CLIENT_ID/_SECRET` exported) so shot 1 shows no JSON upload.
- Clear keychain entry `be.abhishek.notare.secure-store / google-calendar / oauth` first so
  the consent screen actually appears.
- Set macOS/Windows display to 1920×1080 so the URL text is readable after YouTube compression.

## 3. Privacy policy paragraph (required by the "Limited Use" policy)

Put this (or equivalent) on `notare.dev/privacy`:

> **Google user data.** Notare's use of information received from Google APIs adheres to
> the Google API Services User Data Policy, including the Limited Use requirements. Notare
> reads calendar and event data only to display upcoming meetings and prefill notes on your
> device. This data is stored locally on your computer, is never transmitted to Notare or
> any third party, is not used for advertising, and is deleted when you disconnect your
> account or uninstall the app.

## 4. After approval

- Nothing to ship: the client ID does not change. Existing testing-mode users just stop
  being logged out every 7 days.
- Update `docs/GOOGLE-CALENDAR.md` to drop the "unverified app" section.
- Record the approval date in `docs/release/` for the next gate doc.
