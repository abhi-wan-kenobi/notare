import { useLiveQuery } from "~/db";

type SeriesSqlRow = {
  series_id: string;
  meeting_count: number;
  first_started_at: string;
  latest_started_at: string;
  previous_started_at: string;
};

export type SessionSeriesMeta = {
  seriesId: string;
  meetingCount: number;
  firstStartedAt: string;
  latestStartedAt: string;
  /** Null until the series has two occurrences to measure a gap between. */
  cadence: "Weekly" | "Biweekly" | "Monthly" | null;
};

export function useSessionSeriesMeta(
  sessionId: string,
): SessionSeriesMeta | null {
  const { data = null } = useLiveQuery<SeriesSqlRow, SessionSeriesMeta | null>({
    sql: `
      SELECT
        series_id,
        COUNT(*) AS meeting_count,
        MIN(COALESCE(NULLIF(started_at, ''), created_at)) AS first_started_at,
        MAX(COALESCE(NULLIF(started_at, ''), created_at)) AS latest_started_at,
        (
          SELECT COALESCE(NULLIF(previous.started_at, ''), previous.created_at)
          FROM sessions AS previous
          WHERE previous.series_id = current.series_id
            AND previous.deleted_at IS NULL
          ORDER BY COALESCE(NULLIF(previous.started_at, ''), previous.created_at) DESC, previous.id DESC
          LIMIT 1 OFFSET 1
        ) AS previous_started_at
      FROM sessions AS current
      WHERE current.series_id <> ''
        AND current.deleted_at IS NULL
        AND current.series_id = (
          SELECT series_id FROM sessions WHERE id = ? AND deleted_at IS NULL
        )
      GROUP BY series_id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => {
      const row = rows[0];
      if (!row?.series_id) return null;
      const gapDays =
        row.previous_started_at && row.latest_started_at
          ? Math.abs(
              Date.parse(row.latest_started_at) -
                Date.parse(row.previous_started_at),
            ) / 86_400_000
          : null;
      const cadence =
        gapDays === null || !Number.isFinite(gapDays)
          ? null
          : gapDays >= 24 && gapDays <= 38
            ? "Monthly"
            : gapDays >= 11 && gapDays <= 17
              ? "Biweekly"
              : gapDays >= 5 && gapDays <= 9
                ? "Weekly"
                : null;
      return {
        seriesId: row.series_id,
        meetingCount: Number(row.meeting_count),
        firstStartedAt: row.first_started_at,
        latestStartedAt: row.latest_started_at,
        cadence,
      };
    },
  });
  return sessionId ? data : null;
}
