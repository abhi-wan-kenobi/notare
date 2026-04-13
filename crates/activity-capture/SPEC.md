# Activity Capture Spec

This spec defines the high-level contract for `activity-capture-interface`, `activity-capture-macos`, and `activity-capture`.

It intentionally describes invariants and system boundaries rather than concrete field layouts or timer-by-timer behavior. The source of truth for those details is the code.

## Goal

Activity capture should preserve enough context to reconstruct what the user was doing while keeping screenshot capture and local vision-language inference sparse.

The system is built around one idea: the durable unit is a logical **observation**, not every poll and not every text change.

## Core Invariants

1. Raw snapshots are cheap, frequent, and disposable.
2. Observations are durable spans of meaning such as editing one draft, reading one page, or working in one window.
3. Text is the primary high-signal stream. Screenshots are supporting artifacts.
4. Stable text-anchor identity keeps ongoing typing inside the same observation.
5. Structural context changes create new observations.
6. High-frequency runtime state lives in memory. Durable storage stays append-only.
7. Vision analysis runs only for accepted screenshot artifacts, never for every raw update.

## System Shape

The pipeline is:

1. Platform capture produces normalized raw snapshots.
2. An observation reducer folds snapshots into logical observation spans.
3. An artifact planner decides whether a screenshot is worth taking.
4. Analysis produces observation-oriented summaries from accepted artifacts.

Each stage should stay narrow in responsibility:

- platform capture extracts and sanitizes raw signals
- the reducer decides continuity vs replacement
- artifact planning controls screenshot cost
- analysis describes accepted evidence, not the live stream

## Observation Semantics

An observation represents one continuous user activity in one stable context.

The system should keep a single observation alive while the user is still operating in the same logical place, especially while typing into the same anchored input. Text churn alone is not enough to create a new durable activity.

A new observation should start when the activity context materially changes, such as:

- a different app becomes active
- the relevant window or container changes
- browser navigation changes the page context
- the focused text anchor changes to a different logical input
- the previous observation ends due to inactivity or loss of signal

## Screenshot And Analysis Policy

Screenshots are not the sampling primitive. They are selective evidence.

The system should prefer:

- a contextual screenshot near observation start when useful
- a representative screenshot after typing or interaction settles
- occasional low-frequency refreshes for long-lived observations

The system should avoid repeated near-duplicate screenshots while the user is actively typing in the same place.

For drafting flows, the desired durable result is usually:

- the latest text state
- one representative UI view
- one analysis attached to that accepted artifact

## Persistence Model

Persistence is append-only.

The database stores:

- observation lifecycle events
- accepted screenshot artifacts
- analysis records derived from those artifacts

The current or preferred view of an observation is a read-model concern derived from append-only records plus in-memory runtime state. The database is not responsible for mutable session-like state such as debounce windows, pending timers, or the current draft-in-progress.

## Read-Model Principle

Consumers should read observations, not raw capture chatter.

When multiple artifacts exist for one observation, downstream consumers should prefer the most representative artifact for that observation rather than treating every screenshot or every analysis as equally important.

This keeps the product aligned with user intent: one activity should read like one coherent unit.

## Cross-Crate Boundaries

### `activity-capture-interface`

Defines the normalized raw capture contract shared across platforms. It should stay focused on representing capture inputs, not on owning observation lifecycle policy.

### `activity-capture-macos`

Owns extraction and sanitization of macOS-specific signals. Its job is to provide the best stable raw context it can, especially around focused element identity, while remaining agnostic about higher-level observation policy.

### `activity-capture`

Owns observation reduction, screenshot planning, and the orchestration that turns raw capture into durable observation artifacts.

## Non-Goals

This system does not try to:

- preserve every keystroke as its own durable event
- capture screenshots on every poll or every text change
- make screenshots the primary source of truth
- encode long-lived mutable activity state directly in the database

## Design Bias

When there is tension between completeness and cost, prefer designs that keep the logical observation correct and make artifacts sparse but meaningful.

Missing some intermediate visual churn is acceptable. Splitting one continuous activity into noisy fragments is usually worse.
