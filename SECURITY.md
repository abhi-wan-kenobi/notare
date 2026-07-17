# Security Policy

## Supported versions

Only the **latest release** of Notare receives security fixes. If you are on an
older version, please update to the current release before reporting.

| Version | Supported |
| ------- | --------- |
| Latest release | Yes |
| Older releases | No |

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Instead, report privately through GitHub Security Advisories:

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**.
3. Fill in the advisory form with as much detail as you can (affected
   version/platform, reproduction steps, impact).

Direct link: <https://github.com/abhi-wan-kenobi/notare/security/advisories/new>

## What to expect

- **Acknowledgement** within 7 days.
- An initial **assessment** (accepted / needs more info / not a vulnerability)
  within 14 days.
- Notare is maintained by a solo developer, so fix timelines depend on
  severity: issues that expose audio, transcripts, or notes are treated as
  highest priority.
- You will be credited in the advisory and release notes unless you ask
  otherwise.

## Scope notes

- Notare is a **local-first** app: transcription and notes stay on your
  machine. Anything that silently sends user content off-device is considered
  a critical vulnerability.
- The update pipeline is signed. Issues with update-signature verification are
  in scope and high priority.
