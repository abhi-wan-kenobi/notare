# db-app Spec

`db-app` is the durable per-day artifact store.

It intentionally does not store raw activity capture data. That remains in
`db-activity`.

## Tables

### `daily_notes`

One row per user per date for the canonical daily note/editor document.

Columns:
- `id`
- `date`
- `body`
- `user_id`
- `visibility`
- `created_at`
- `updated_at`

Unique:
- `(date, user_id)`

### `daily_summaries`

One derived summary artifact per daily note. This is what the daily summary UI
should render.

Columns:
- `id`
- `daily_note_id`
- `date`
- `content`
- `timeline_json`
- `topics_json`
- `status`
- `source_cursor_ms`
- `source_fingerprint`
- `generation_error`
- `generated_at`
- `created_at`
- `updated_at`

Unique:
- `daily_note_id`

## Model

- `daily_notes` is user-authored canonical content.
- `daily_summaries` is machine-generated durable output.
- Raw screenshots, transitions, and VLM output do not belong here.
