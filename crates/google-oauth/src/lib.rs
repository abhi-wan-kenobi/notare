//! Installed-app ("bring your own client") Google OAuth for Notare.
//!
//! Users create their own Desktop-app OAuth client in the Google Cloud
//! console (see `docs/GOOGLE-CALENDAR.md`), import the downloaded
//! `client_secret_*.json` into Notare, and this crate runs the standard
//! installed-app flow: consent URL → loopback redirect on 127.0.0.1 →
//! code exchange (with PKCE) → offline refresh token.
//!
//! No Notare-owned cloud or vendor secret is involved anywhere.

mod client_json;
mod error;
mod flow;
mod token;

pub use client_json::{
    ClientCredentials, ClientJsonKind, DEFAULT_AUTH_URI, DEFAULT_TOKEN_URI, parse_client_json,
};
pub use error::Error;
pub use flow::{
    DEFAULT_SCOPES, PkcePair, RedirectOutcome, build_auth_url, connect, generate_pkce,
    parse_redirect_request, pkce_challenge_s256,
};
pub use token::{
    DEFAULT_REVOKE_URI, TokenResponse, exchange_code, refresh_access_token, revoke_token,
};
