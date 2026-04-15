# db-app Spec

`db-app` is the local desktop SQLite store.

## Tables

### `templates`

User-authored note templates mirrored into the desktop SQLite database.

Columns:
- `id`
- `title`
- `description`
- `pinned`
- `pin_order`
- `category`
- `targets_json`
- `sections_json`
- `created_at`
- `updated_at`

Unique:
- `id`

### `calendars`

Connected calendar metadata cached in the desktop SQLite database.

Columns:
- `id`
- `tracking_id_calendar`
- `name`
- `enabled`
- `provider`
- `source`
- `color`
- `connection_id`
- `created_at`
- `updated_at`

Unique:
- `id`

### `events`

Calendar events mirrored into the desktop SQLite database.

Columns:
- `id`
- `tracking_id_event`
- `calendar_id`
- `title`
- `started_at`
- `ended_at`
- `location`
- `meeting_link`
- `description`
- `note`
- `recurrence_series_id`
- `has_recurrence_rules`
- `is_all_day`
- `provider`
- `participants_json`
- `created_at`
- `updated_at`

Unique:
- `id`

## Model

- `templates` is the durable local store for user templates.
- `calendars` stores synced calendar metadata for local app reads.
- `events` stores synced calendar events for local app reads.
