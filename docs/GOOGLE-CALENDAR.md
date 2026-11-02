# Connect Google Calendar to Notare

Notare talks to Google Calendar **directly from your computer**. Even when using the bundled official client, there is no Notare cloud, no intermediate server, and no account with us: Google talks only to the app on `127.0.0.1`, and your calendar data never passes through `notare.dev` or anyone else's servers.

All requested scopes are **read-only**: Notare can view your calendars and events, but it cannot create, modify, or delete anything on your Google Calendar, and it never sees your Google password.

---

## Quick connect (official builds)

Official builds of Notare ship with a first-party Google OAuth client baked in (`bundled_client()`), so connecting is a single click:

1. In Notare, go to **Settings → Calendar** (also accessible from the calendar sidebar and during onboarding).
2. Under the **Google** section, click **"Sign in with Google"**.
3. Your default web browser opens Google's OAuth consent screen for **Notare**.
4. Choose your Google account and grant the requested calendar permissions (`calendar.readonly` and `calendar.events.readonly`).
5. The browser displays **"Google Calendar connected — you can close this tab"**.
6. Back in Notare, your calendars appear within a few seconds. **Tick the calendars you want** to sync.

Multiple calendars and multiple Google accounts are supported:
- **Multiple accounts**: Connect both work and personal accounts. Click **"+ Add Google account"** to authenticate an additional account. Refresh tokens are isolated per account in your OS keychain under `google-calendar/oauth/<account_email>`.
- **Per-account controls**: Disconnect specific accounts individually or remove all at once.
- **Incremental write permission**: Initial connection requests read-only calendar access. If you choose to create an event directly from Notare, Google will prompt you incrementally for write permission (`calendar.events`) without requiring full re-authorization.
### Unverified app warning

Until Google finishes the OAuth verification process for our first-party client, you may see a screen saying:

> **"Google hasn't verified this app"**

This is normal while an app is under Google verification or operating in testing mode. You can proceed safely:
1. Click **Advanced** (in small text near the bottom left).
2. Click **Go to Notare (unsafe)**.
3. Review the permissions and click **Continue**.

> **Testing mode token expiry:** While in Google testing mode, refresh tokens expire after **7 days**. If Notare reports that it can no longer refresh your calendar events, simply return to **Settings → Calendar** and click **Sign in with Google** again to re-authenticate. Once Google verification is complete, refresh tokens remain valid without the 7-day expiration.

### Accounts & Disconnecting

- **Add another account**: Click **"+ Add Google account"** in the Google Calendar settings to connect multiple accounts.
- **Remove account**: Click **Remove** next to any account in the Connected Accounts list to revoke its session and delete its token from your OS keychain.
- **Disconnect all**: Revokes and removes all connected Google accounts.
---

## Bring your own OAuth client (forks, source builds, or if you prefer it)

If you are running a source build or fork (which do not ship with the bundled client credentials), or if you prefer to have your calendar authentication tied to your own personal Google Cloud project, you can bring your own OAuth client.

The one-time setup takes about 10 minutes: you create a (free) credential in Google's developer console, download a small JSON file, and hand it to Notare. After that, connecting is a single click.

### Part 1 — Create your Google OAuth client (one time)

You need a Google account. Everything below is free and does not require a credit card.

#### 1. Create a project

1. Open [https://console.cloud.google.com/projectcreate](https://console.cloud.google.com/projectcreate) (sign in if asked).
2. Project name: anything you like, e.g. `notare`.
3. Click **Create**, and wait a few seconds for the notification.
4. Make sure the new project is selected in the project picker at the top of the page (it usually selects itself).

#### 2. Enable the Google Calendar API

1. Open [https://console.cloud.google.com/apis/library/calendar-json.googleapis.com](https://console.cloud.google.com/apis/library/calendar-json.googleapis.com).
2. Check the project name shown at the top is your new project.
3. Click **Enable**.

#### 3. Set up the consent screen

1. Open [https://console.cloud.google.com/auth/overview](https://console.cloud.google.com/auth/overview) (called "Google Auth Platform"). If Google asks you to configure it, click **Get started**.
2. **App information**: App name `Notare` (or anything), pick your own email as the support email. Click **Next**.
3. **Audience**: choose **External**. Click **Next**.
4. **Contact information**: your email again. Click **Next**, agree, then **Create**.
5. Now add yourself as a test user: go to **Audience** in the left menu, scroll to **Test users**, click **+ Add users**, enter the Gmail address (or addresses) whose calendars you want to connect, and **Save**.

> Your app stays in "Testing" mode forever — that's fine and intended. It just means only the test users you listed can connect, which is exactly what you want. (Google shows an "unverified app" notice during consent; that's normal for personal clients.) Note: Google expires refresh tokens for testing-mode apps after about 6 months of inactivity — if Notare ever says it can't refresh, just click Connect again.

#### 4. Create the Desktop-app OAuth client

1. Open [https://console.cloud.google.com/auth/clients](https://console.cloud.google.com/auth/clients) (or **Clients** in the left menu of the Google Auth Platform page).
2. Click **+ Create client**.
3. **Application type**: choose **Desktop app** ← important, not "Web".
4. Name: `Notare desktop` (anything works).
5. Click **Create**.

#### 5. Download the JSON

In the dialog that appears (or via the download icon next to the client in the list), click **Download JSON**. You'll get a file named something like:

```
client_secret_1234567890-abcdefg.apps.googleusercontent.com.json
```

Save it anywhere you can find it (Downloads is fine). Treat it like a password — don't share it or commit it to a public repo.

That's the end of the Google console part. You never need to go back there (unless you want to add another test user).

---

### Part 2 — Connect in Notare

1. In Notare, go to **Settings → Calendar** (or the calendar sidebar / onboarding) and expand the **Google** section.
   - If using an official build with a bundled client, click **"Advanced: use your own OAuth client"** to reveal the client import UI.
   - On source or fork builds without a bundled client, the import UI is shown directly.
2. Click **"Select your Google client JSON…"** and pick the file you just downloaded. (You can also paste the JSON text instead.)
   - Notare reads the client id/secret out of the file and stores them in your **operating system's keychain** — not in a plain-text config file.
3. Click **"Connect Google Calendar"**. Your browser opens the Google consent screen:
   - pick the account you added as a test user,
   - click **Continue** past the "Google hasn't verified this app" notice (it's *your* app — Advanced → Go to Notare if needed),
   - allow the two read-only calendar permissions.
4. The browser shows "Google Calendar connected — you can close this tab". Back in Notare, your calendars appear within a few seconds.
5. **Tick the calendars you want** to sync. Multiple calendars are supported — each toggle is independent. Events from enabled calendars flow into the calendar view, meeting notes, and upcoming-meeting notifications.

#### Disconnecting

- **Disconnect** (in the Google section) revokes and forgets the session but keeps your imported client JSON, so reconnecting is one click.
- **Disconnect & remove client** (or clicking **Reset / Remove client**) also deletes the stored client id/secret from your keychain.

---

## What Notare stores and requests

| Item | Where |
| --- | --- |
| Per-account refresh tokens | OS keychain (`google-calendar/oauth/<account_email>`) |
| Client id + client secret | OS keychain (macOS Keychain / Windows Credential Manager / Secret Service on Linux) |
| Which calendars are enabled | Notare's local database (per-account connection ids `google:<email>`) |
| Access tokens | Memory only, refreshed automatically per account |

Scopes requested:
- Initial read-only scopes:
  - `https://www.googleapis.com/auth/calendar.readonly`
  - `https://www.googleapis.com/auth/calendar.events.readonly`
  - `https://www.googleapis.com/auth/userinfo.email`
- Incremental write scope (requested only when creating an event):
  - `https://www.googleapis.com/auth/calendar.events`
---

## Troubleshooting

- **"Google hasn't verified this app" in the browser** — see the [Unverified app warning](#unverified-app-warning) section above. Click **Advanced → Go to Notare (unsafe)** to continue.
- **"Error 403: access_denied" in the browser** — when using your own OAuth client in Testing mode, the Google account you picked isn't in the consent screen's **Test users** list. Add it (Part 1, step 3.5) and retry.
- **"This looks like a Web application client" warning in Notare** — you created the wrong client type. Create a new client with type **Desktop app** (Part 1, step 4) and import that JSON instead.
- **"redirect_uri_mismatch"** — same cause as above (Web clients don't allow loopback redirects). Use a Desktop-app client.
- **Nothing happens after clicking Connect** — check your default browser; the consent tab sometimes opens behind other windows. The connect attempt waits 5 minutes, then you can retry.
- **"access-token refresh failed" or prompt to re-authenticate** — in testing mode, Google expires refresh tokens after 7 days (or after months of inactivity for personal BYO projects). Go to **Settings → Calendar** and click **Sign in with Google** / **Connect** again.
- **Calendar list is empty** — click the refresh icon in the calendar list, or toggle the section closed/open. Also confirm the Calendar API is enabled (Part 1, step 2).
