-- 0020_event_summaries_post_event.sql
-- Plan 008 (Phase 1): Event lifecycle — post-event summary snapshots.
--
-- NOTE: Plan 008 originally documented this as migration 0019, but that slot
-- was taken by 0019_event_poster.sql (Plan 009). Renumbered to 0020.
--
-- Phase 1 adds ONLY the event_summaries table + the post-event registration
-- gating columns. Recap content columns (recap_*) live on the same table so
-- Phase 2 (public recap) can populate them without a second migration; they
-- default to empty/draft and are unused until Phase 2.

-- ============================================================
-- EVENT_SUMMARIES — frozen point-in-time snapshot per event
-- ============================================================
-- One row per event, written once at freeze time (lazy on first
-- read after event_end_ms, or via manual POST /summary/freeze).
-- The freeze captures the funnel + financials AS THEY WERE at
-- freeze time. Later refunds/claims do NOT mutate this row.
CREATE TABLE IF NOT EXISTS event_summaries (
    event_id              TEXT PRIMARY KEY,
    -- Funnel snapshot
    registered_count      INTEGER NOT NULL,
    deposited_count       INTEGER NOT NULL,   -- verified USDC + THB combined
    checked_in_count      INTEGER NOT NULL,
    no_show_count         INTEGER NOT NULL,   -- registered, not checked in
    claimed_count         INTEGER NOT NULL,
    refunded_count        INTEGER NOT NULL,
    post_event_reg_count  INTEGER NOT NULL DEFAULT 0,
    -- Financials (atomic units: 1 USDC = 1_000_000, THB in satang)
    usdc_deposited_total  INTEGER NOT NULL,
    usdc_refunded_total   INTEGER NOT NULL,
    thb_deposited_total   INTEGER NOT NULL,
    thb_refunded_total    INTEGER NOT NULL,
    -- Stability — copy event time bounds at freeze so the snapshot
    -- is interpretable even if the event row is later edited.
    event_start_ms        INTEGER NOT NULL,
    event_end_ms          INTEGER NOT NULL,
    frozen_at             TEXT NOT NULL,      -- ISO 8601
    frozen_by             TEXT NOT NULL DEFAULT '',  -- email; '' = auto
    -- Recap content (Phase 2; unused in Phase 1)
    recap_markdown        TEXT NOT NULL DEFAULT '',
    recap_image_url       TEXT NOT NULL DEFAULT '',
    recap_published_at    TEXT,               -- NULL = draft
    -- Extensibility — per-format breakdowns, top-N stats, etc.
    -- v1 shape: {"by_format": {"in_person": N, "online": M}, "top_roles": [...]}
    -- Left as '{}' until Phase 2+ populates it.
    breakdown_json        TEXT NOT NULL DEFAULT '{}',
    created_at            TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at            TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================
-- EVENTS — new columns for post-event surfaces
-- ============================================================
-- Added in Phase 1 (gating) but only exercised by Phase 3 (post-event
-- registration) and Phase 2 (recap visibility). Defaults are safe for
-- all existing rows: registration closed, recap unpublished.
ALTER TABLE events ADD COLUMN post_event_registration_open   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN post_event_registration_until_ms INTEGER;  -- NULL = no deadline
ALTER TABLE events ADD COLUMN recap_published                 INTEGER NOT NULL DEFAULT 0;

-- ============================================================
-- ATTENDEES — distinguish pre-event vs post-event registrations
-- ============================================================
-- registration_phase is orthogonal to approval_status and
-- participation_type. Pre-event = normal registration flow.
-- Post-event = registered interest AFTER event_end_ms (no
-- deposit, no check-in, no capacity impact). Existing rows are
-- 'pre_event' by default.
ALTER TABLE attendees ADD COLUMN registration_phase TEXT NOT NULL DEFAULT 'pre_event';

CREATE INDEX IF NOT EXISTS idx_attendees_phase ON attendees(event_id, registration_phase);
