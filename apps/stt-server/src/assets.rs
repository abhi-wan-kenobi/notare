//! Embedded web admin page (Phase 3, `docs/stt-server-design.md` §9 + §11).
//!
//! A single self-contained static HTML document — inline CSS, inline vanilla
//! JS, no build step, no CDN, no framework — embedded into the binary via
//! `include_str!` and served at `GET /` by [`crate::admin::web::index`]. This
//! deliberately collapses the design doc's `assets/{index.html,app.js,
//! app.css}` sketch into one file: the whole admin surface is small enough
//! (three read-mostly panels) that splitting it buys nothing and would add a
//! second embedded-asset route to maintain.

/// The admin page. Talks to `/api/status`, `/api/models` and the Phase 2
/// model-mutation routes (currently `501`, handled gracefully client-side —
/// see the `Api` object in the embedded `<script>`).
pub const INDEX_HTML: &str = include_str!("assets/index.html");
