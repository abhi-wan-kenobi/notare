# `db-live-query`

## Role

- Reusable live-query runtime over arbitrary SQL.
- Owns query execution, dependency analysis, subscription state, rerun targeting, and sink delivery.
- Consumes raw observed table-change signals from `db-core2` and maps them onto canonical dependency targets.

## Owns

- `DbRuntime` and the background dispatcher.
- Schema-aware dependency analysis via `extract_dependencies(...)`.
- Schema catalog loading/caching from SQLite metadata.
- Canonical dependency targets for ordinary tables and supported virtual tables.
- Raw-table to dependency-target canonicalization for rerun targeting.
- Subscription registration, activation, refresh, and unregistration.
- Row JSON serialization and stale-sink cleanup.

## Does Not Own

- SQLite pool creation, raw hooks, or DB open policy.
- App DB bootstrap, file paths, or migrations.
- Tauri commands, channels, JS bindings, or React hooks.
- Row-level or predicate-level invalidation.

## Invariants

- Keep this crate transport-agnostic.
- `db-core2` stays schema-agnostic and emits raw observed table changes only.
- `db-live-query` is the layer that interprets schema, resolves dependencies, and decides rerun targeting.
- Dependency analysis must be explicit: `Reactive { targets }` or `NonReactive { reason }`.
- Non-reactive subscriptions still deliver the initial result or error; they simply never auto-refresh.
- Reactive dependencies are canonical targets, not raw observed table names.
- Supported ordinary tables map to `DependencyTarget::Table`.
- Supported virtual tables map to `DependencyTarget::VirtualTable`.
- FTS5 is supported through virtual-table/shadow-table canonicalization.
- Unsupported or unresolvable dependencies must make the whole subscription non-reactive.
- Empty dependency extraction must make the subscription non-reactive.
- Partial dependency sets are not allowed.
- Views are not watched directly; they are only reactive when query-plan expansion reaches supported underlying targets.
- The first event for every subscription is the initial snapshot or initial error for that exact SQL + params pair.
- Dispatcher-driven refreshes must not emit before the first event is delivered.
- If writes land while a reactive subscription is initializing, they collapse into at most one catch-up refresh after activation.
- Catch-up refreshes that would deliver the same payload as the initial event must be suppressed.
- Refresh jobs whose triggering sequence is at or below the subscription's activation ignore floor must not emit.
- `unsubscribe()` resolving is a hard delivery barrier: no later event may be delivered after it returns.
- Refresh delivery must be revalidated against current subscription state immediately before sending.
- Stale subscribers must be removed when sink delivery fails.
- Reruns remain dependency-target-granular only; no incremental view maintenance.

## Dependency Direction

- May depend on `db-core2`.
- May be consumed by `plugins/db` and `mobile-bridge`.
- Must not depend on Tauri or app-specific UI/runtime layers.

## Test Ownership

- Keep dependency-analysis, canonicalization, rerun targeting, init-time catch-up, unsubscribe barriers, stale-sink cleanup, and JSON serialization tests here.
- Higher layers should only add thin adapter tests.
