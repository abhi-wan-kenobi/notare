# Connect Google Calendar to Notare

Notare talks to Google Calendar **directly from your computer**, using a
Google "OAuth client" that belongs to *you*. There is no Notare cloud, no
account with us, and your calendar data never passes through anyone else's
servers.

The one-time setup takes about 10 minutes: you create a (free) credential in
Google's developer console, download a small JSON file, and hand it to Notare.
After that, connecting is a single click.

> **Why do I have to do this myself?** Apps normally ship a shared OAuth
> client owned by the app vendor — which means the vendor sits in the middle.
> Open-source, local-first tools (rclone, gcalcli, and now Notare) instead ask
> you to bring your own client, so the only parties involved are you and
> Google. The scopes Notare requests are **read-only**.

---

## Part 1 — Create your Google OAuth client (one time)

You need a Google account. Everything below is free and does not require a
credit card.

### 1. Create a project

1. Open <https://console.cloud.google.com/projectcreate> (sign in if asked).
2. Project name: anything you like, e.g. `notare`.
3. Click **Create**, and wait a few seconds for the notification.
4. Make sure the new project is selected in the project picker at the top of
   the page (it usually selects itself).

### 2. Enable the Google Calendar API

1. Open <https://console.cloud.google.com/apis/library/calendar-json.googleapis.com>.
2. Check the project name shown at the top is your new project.
3. Click **Enable**.

### 3. Set up the consent screen

1. Open <https://console.cloud.google.com/auth/overview> (called
   "Google Auth Platform"). If Google asks you to configure it, click
   **Get started**.
2. **App information**: App name `Notare` (or anything), pick your own email
   as the support email. Click **Next**.
3. **Audience**: choose **External**. Click **Next**.
4. **Contact information**: your email again. Click **Next**, agree, then
   **Create**.
5. Now add yourself as a test user: go to **Audience** in the left menu,
   scroll to **Test users**, click **+ Add users**, enter the Gmail address
   (or addresses) whose calendars you want to connect, and **Save**.

> Your app stays in "Testing" mode forever — that's fine and intended. It just
> means only the test users you listed can connect, which is exactly what you
> want. (Google shows an "unverified app" notice during consent; that's
> normal for personal clients.) Note: Google expires refresh tokens for
> testing-mode apps after about 6 months of inactivity — if Notare ever says
> it can't refresh, just click Connect again.

### 4. Create the Desktop-app OAuth client

1. Open <https://console.cloud.google.com/auth/clients> (or **Clients** in the
   left menu of the Google Auth Platform page).
2. Click **+ Create client**.
3. **Application type**: choose **Desktop app** ← important, not "Web".
4. Name: `Notare desktop` (anything works).
5. Click **Create**.

### 5. Download the JSON

In the dialog that appears (or via the download icon next to the client in
the list), click **Download JSON**. You'll get a file named something like:

```
client_secret_1234567890-abcdefg.apps.googleusercontent.com.json
```

Save it anywhere you can find it (Downloads is fine). Treat it like a
password — don't share it or commit it to a public repo.

That's the end of the Google console part. You never need to go back there
(unless you want to add another test user).

---

## Part 2 — Connect in Notare

1. In Notare, open the **calendar sidebar** (also offered during onboarding)
   and expand the **Google** section.
2. Click **"Select your Google client JSON…"** and pick the file you just
   downloaded. (You can also paste the JSON text instead.)
   - Notare reads the client id/secret out of the file and stores them in
     your **operating system's keychain** — not in a plain-text config file.
3. Click **"Connect Google Calendar"**. Your browser opens the Google consent
   screen:
   - pick the account you added as a test user,
   - click **Continue** past the "Google hasn't verified this app" notice
     (it's *your* app — Advanced → Go to Notare if needed),
   - allow the two read-only calendar permissions.
4. The browser shows "Google Calendar connected — you can close this tab".
   Back in Notare, your calendars appear within a few seconds.
5. **Tick the calendars you want** to sync. Multiple calendars are supported —
   each toggle is independent. Events from enabled calendars flow into the
   calendar view, meeting notes, and upcoming-meeting notifications.

### Disconnecting

- **Disconnect** (in the Google section) revokes and forgets the session but
  keeps your imported client JSON, so reconnecting is one click.
- **Disconnect & remove client** (right-click a calendar group) also deletes
  the stored client id/secret from your keychain.

---

## What Notare stores and requests

| Item | Where |
| --- | --- |
| Client id + client secret + refresh token | OS keychain (macOS Keychain / Windows Credential Manager / Secret Service on Linux) |
| Which calendars are enabled | Notare's local database |
| Access tokens | Memory only, refreshed automatically |

Scopes requested (both read-only):

- `https://www.googleapis.com/auth/calendar.readonly`
- `https://www.googleapis.com/auth/calendar.events.readonly`

Notare never writes to your Google Calendar and never sees your Google
password. The OAuth redirect happens on `127.0.0.1` (your own machine), the
standard flow for desktop apps.

---

## Troubleshooting

- **"Error 403: access_denied" in the browser** — the Google account you
  picked isn't in the consent screen's **Test users** list. Add it
  (Part 1, step 3.5) and retry.
- **"This looks like a Web application client" warning in Notare** — you
  created the wrong client type. Create a new client with type
  **Desktop app** (Part 1, step 4) and import that JSON instead.
- **"redirect_uri_mismatch"** — same cause as above (Web clients don't allow
  loopback redirects). Use a Desktop-app client.
- **Nothing happens after clicking Connect** — check your default browser; the
  consent tab sometimes opens behind other windows. The connect attempt waits
  5 minutes, then you can retry.
- **"access-token refresh failed" after months of use** — Google expired the
  refresh token (testing-mode apps expire after ~6 months of inactivity, and
  Google caps a client at 100 outstanding refresh tokens). Click **Connect**
  again.
- **Calendar list is empty** — click the refresh icon in the calendar list, or
  toggle the section closed/open. Also confirm the Calendar API is enabled
  (Part 1, step 2).
