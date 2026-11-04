import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import type * as Sqlite from "node:sqlite";
import type { DatabaseSync, SQLInputValue } from "node:sqlite";

// Vite's resolver does not know `node:sqlite` yet, so a static import fails
// to bundle; a CommonJS require is resolved by Node itself.
const { DatabaseSync: SqliteDatabase } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof Sqlite;

const MIGRATIONS_DIR = join(__dirname, "../../../../crates/db-app/migrations");

// Applies the real db-app migrations so tests run the app's SQL against the
// shipped schema. The sqlite-vec migration needs the `vec0` extension, which
// plain SQLite lacks, and nothing under test touches it.
export function createMigratedDb(): DatabaseSync {
  const db = new SqliteDatabase(":memory:");
  for (const file of readdirSync(MIGRATIONS_DIR)
    .filter((name) => name.endsWith(".sql"))
    .sort()) {
    try {
      db.exec(readFileSync(join(MIGRATIONS_DIR, file), "utf8"));
    } catch (error) {
      if (!String(error).includes("vec0")) throw error;
    }
  }
  return db;
}

export function queryAll<Row>(
  db: DatabaseSync,
  sql: string,
  params: readonly unknown[] = [],
): Row[] {
  return db
    .prepare(sql)
    .all(...(params as SQLInputValue[]))
    .map((row) => ({ ...row })) as Row[];
}

export function execAll(
  db: DatabaseSync,
  statements: ReadonlyArray<{ sql: string; params?: readonly unknown[] }>,
) {
  for (const statement of statements) {
    db.prepare(statement.sql).run(
      ...((statement.params ?? []) as SQLInputValue[]),
    );
  }
}

export function insertCalendar(
  db: DatabaseSync,
  calendar: { id: string; provider: string; enabled?: boolean },
) {
  db.prepare(
    `INSERT INTO calendars (id, name, enabled, provider, created_at, updated_at)
     VALUES (?, ?, ?, ?, '2026-11-01T00:00:00.000Z', '2026-11-01T00:00:00.000Z')`,
  ).run(
    calendar.id,
    `${calendar.provider} calendar`,
    calendar.enabled === false ? 0 : 1,
    calendar.provider,
  );
}

export function insertEvent(
  db: DatabaseSync,
  event: {
    id: string;
    calendarId: string;
    title?: string;
    startedAt: string;
  },
) {
  db.prepare(
    `INSERT INTO events (id, calendar_id, title, started_at, ended_at, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, '2026-11-01T00:00:00.000Z', '2026-11-01T00:00:00.000Z')`,
  ).run(
    event.id,
    event.calendarId,
    event.title ?? event.id,
    event.startedAt,
    event.startedAt,
  );
}
