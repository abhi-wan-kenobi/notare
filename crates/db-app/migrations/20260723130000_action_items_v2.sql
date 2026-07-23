-- Structured action items v2 (WS-C). Adds the anti-hallucination + push
-- provenance columns to `action_items`. All additive with STRICT-compatible
-- non-null defaults so existing rows and existing column-list inserts are
-- unaffected (legacy_import / session_ops / cloudsync / task-storage all use
-- explicit column lists, audited 2026-07-23).

-- Model self-reported confidence in [0,1]. 0 for legacy/manually-entered rows.
ALTER TABLE action_items ADD COLUMN confidence REAL NOT NULL DEFAULT 0;

-- VERBATIM substring of the normalized transcript that produced this item.
-- The extraction pipeline rejects any item whose source_text is not an exact
-- substring of the transcript (structural anti-hallucination gate), so a
-- non-empty value here is a provenance guarantee, not a paraphrase.
ALTER TABLE action_items ADD COLUMN source_text TEXT NOT NULL DEFAULT '';

-- Millisecond offset into the recording where source_text begins (matched
-- against words_json). NULL when unknown / note-derived.
ALTER TABLE action_items ADD COLUMN source_start_ms INTEGER;

-- Diarization speaker id of the owner (from speaker_hints_json / participants),
-- or '' when the owner could not be resolved to a roster member. Distinct from
-- assignee_human_id, which is a resolved contact; owner_speaker_id is the raw
-- diarized speaker the extractor attributed the item to.
ALTER TABLE action_items ADD COLUMN owner_speaker_id TEXT NOT NULL DEFAULT '';

-- '' | 'low' | 'medium' | 'high'. Free-form-tolerant; '' = unset.
ALTER TABLE action_items ADD COLUMN priority TEXT NOT NULL DEFAULT '';

-- JSON array of push targets this item has been synced to (Obsidian/CSV/
-- webhook), e.g. [{"target":"obsidian","at":"..."}]. WS-D2 read/writes this.
ALTER TABLE action_items ADD COLUMN synced_targets_json TEXT NOT NULL DEFAULT '[]';
