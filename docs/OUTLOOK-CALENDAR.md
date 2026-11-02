# Connect Outlook Calendar to Notare

Notare talks to Microsoft Outlook Calendar **directly from your computer**. Even when using the bundled official client, there is no Notare cloud, no intermediate proxy, and no account with us: Microsoft identity and Graph API talk directly to the app on `http://localhost`, and your calendar data never passes through `notare.dev` or any third-party servers.

All requested initial scopes are **read-only**: Notare can view your calendars and events, but it cannot create, modify, or delete anything on your Outlook Calendar without explicit consent, and it never sees your Microsoft password.

---

## Quick connect (official builds)

Official builds of Notare ship with a first-party Microsoft OAuth client baked in (`NOTARE_MS_CLIENT_ID`), so connecting is a single click:

1. In Notare, go to **Settings → Calendar** (also accessible from the calendar sidebar and during onboarding).
2. Under the **Outlook** section, click **"Sign in with Microsoft"**.
3. Your default web browser opens Microsoft's OAuth consent screen for **Notare**.
4. Choose your Microsoft account (personal `@outlook.com` / `@hotmail.com` or work/school Microsoft 365) and grant the requested calendar permissions (`offline_access`, `User.Read`, and `Calendars.Read`).
5. The browser displays **"Connected — You can close this tab and return to Notare"**.
6. Back in Notare, your calendars appear within a few seconds. **Tick the calendars you want** to sync.

### Multi-account and per-account keychain storage

- **Multiple accounts**: Connect both work (Microsoft 365 / Entra ID) and personal Outlook accounts. Click **"+ Add Microsoft account"** to authenticate an additional account.
- **Per-account isolation**: Refresh tokens and credentials are securely isolated per account in your OS keychain under `outlook-calendar/oauth/<account_email>`. Connection IDs are tracked as `outlook:<account_email>`.
- **Per-account controls**: Disconnect specific accounts individually by clicking **Remove** next to an account, or disconnect all accounts at once.
- **Incremental write permission**: Initial connection requests read-only calendar access. If you choose to create an event directly from Notare, Microsoft will prompt you incrementally for write permission (`Calendars.ReadWrite`) without requiring full re-authorization.

---

## Bring your own OAuth client (forks, source builds, or personal preference)

If you are running a source build or fork (which do not ship with the compiled-in client ID), or if your organization requires using an internal tenant registration, you can bring your own Application (client) ID.

The one-time setup takes about 5 minutes in Microsoft Entra ID (formerly Azure Active Directory). Everything below is free and does not require a paid Azure subscription.

### Register an application in Microsoft Entra

1. Open the [Microsoft Entra admin center](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/CreateApplicationBlade) (or open Azure Portal → **Microsoft Entra ID** → **App registrations** → **New registration**). Sign in with your Microsoft account.
2. In the registration form:
   - **Name**: `Notare` (or any name you prefer).
   - **Supported account types**: Select **"Accounts in any organizational directory (Any Microsoft Entra ID tenant - Multitenant) and personal Microsoft accounts (e.g. Skype, Xbox)"**. This allows both personal Outlook and work/school accounts to sign in.
   - **Redirect URI (optional)**:
     - In the platform dropdown, select **Mobile and desktop applications** (do **not** select "Web").
     - In the URI input, enter:
       ```
       http://localhost
       ```
       *(Note: Microsoft requires `http://localhost` for public desktop clients; Microsoft's identity platform automatically allows loopback redirects to `http://localhost:<port>` on any ephemeral local port at runtime.)*
3. Click **Register**.
4. On the app overview page, locate and copy the **Application (client) ID** (a UUID like `00000000-0000-0000-0000-000000000000`).
5. (Optional verification): Under **Authentication** in the left menu, ensure **Allow public client flows** has "Enable the following mobile and desktop flows" set to **Yes**, and confirm `http://localhost` is listed under **Mobile and desktop applications**.
6. (Optional verification): Under **API permissions**, confirm `User.Read` is present. You do not need to pre-grant admin consent for `Calendars.Read` or `Calendars.ReadWrite` for personal accounts; users grant user-delegated consent when signing in.

### Configure your Client ID in Notare

1. In Notare, navigate to **Settings → Calendar** → **Outlook**.
2. Click **Advanced: use your own Application (client) ID**.
3. Paste your **Application (client) ID** into the input field and click **Save**.
4. Click **Sign in with Microsoft** to initiate the browser authorization flow.

---

## Technical Details

- **Public Native Client**: Operates without a client secret using PKCE (Proof Key for Code Exchange) with `S256` challenge method.
- **Endpoints**:
  - Authorization: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`
  - Token exchange/refresh: `https://login.microsoftonline.com/common/oauth2/v2.0/token`
  - Graph API Base: `https://graph.microsoft.com/v1.0`
- **Identity & User Profile**: Queries `GET /v1.0/me` to determine primary account email, inspecting `mail` with a fallback to `userPrincipalName`.
- **Calendar & Event Sync**:
  - List calendars: `GET /v1.0/me/calendars`
  - List events: `GET /v1.0/me/calendars/{id}/calendarView?startDateTime=...&endDateTime=...`
  - Paging: Automatically resolves `@odata.nextLink` pagination to retrieve all calendar events across date windows.
  - Create event: `POST /v1.0/me/calendars/{id}/events`
- **Keyring Scopes**:
  - `outlook-calendar`
  - Keys: `client` (custom BYO client ID), `accounts` (list of registered emails), `oauth/<email>` (tokens and granted scopes).

---

## Troubleshooting

- **"Redirect URI does not match" error**: Ensure the platform in Entra is **Mobile and desktop applications** and the URI is `http://localhost` (without a path like `/callback` or port number).
- **Work account consent prompt**: If signing in with a corporate or enterprise Microsoft 365 account, your tenant administrator may restrict third-party applications. Contact your IT administrator or register the app directly in your organization's tenant with "Accounts in this organizational directory only".
- **Local port conflict**: Notare binds an ephemeral TCP port on `127.0.0.1` for the loopback callback. If local firewall software blocks loopback connections, allow local port binding for the Notare application.
