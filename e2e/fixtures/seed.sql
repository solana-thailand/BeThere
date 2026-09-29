-- Fixture for the visual + a11y specs (.plans/038 P0-3). Local D1 only:
-- CI loads it with `wrangler d1 execute --local`, then POST /api/events/reseed-kv
-- copies the event into KV (the read path is KV-first).
-- Synthetic data only (example.com); fixed dates so snapshots do not drift.
-- 2030-01-15 12:00 → 15:00 UTC+7.
INSERT OR REPLACE INTO events (
  id, name, slug, status, event_format, event_start_ms, event_end_ms,
  deposit_enabled, deposit_amount_usdc, deposit_amount_thb, location, tagline,
  organizer_emails, sheet_id, sheet_name, staff_sheet_name, capacity,
  total_attendees, created_at, updated_at, link, visibility,
  in_person_capacity, online_capacity, description
) VALUES (
  'e2e-event', 'E2E Builders Night', 'e2e-builders-night', 'active', 'in_person',
  1894683600000, 1894694400000,
  1, 0, 500, 'Bangkok', 'Snapshot fixture event',
  'organizer@example.com', 'e2e-no-sheet', 'Attendees', 'staff', 40,
  3, '2029-12-01T00:00:00Z', '2029-12-01T00:00:00Z', 'https://example.com', 'public',
  40, -1, 'A fixed event for the visual and accessibility specs.'
);

INSERT OR REPLACE INTO attendees (
  id, event_id, email, name, approval_status, participation_type, claim_token,
  deposit_status, deposit_amount_usdc, deposit_amount_thb, created_at, updated_at,
  consent_marketing, registration_phase
) VALUES
  ('e2e-att-01', 'e2e-event', 'e2e-admin@example.com', 'Somchai Example', 'approved', 'in-person',
   '0190e2e0-0000-7000-8000-000000000001', 'none', 0, 0, '2029-12-02 10:00:00', '2029-12-02 10:00:00', 0, 'pre_event'),
  ('e2e-att-02', 'e2e-event', 'user02@example.com', 'Malee Example', 'approved', 'in-person',
   '0190e2e0-0000-7000-8000-000000000002', 'none', 0, 0, '2029-12-02 11:00:00', '2029-12-02 11:00:00', 0, 'pre_event'),
  ('e2e-att-03', 'e2e-event', 'user03@example.com', 'Arthit Example', 'approved', 'in-person',
   '0190e2e0-0000-7000-8000-000000000003', 'none', 0, 0, '2029-12-02 12:00:00', '2029-12-02 12:00:00', 0, 'pre_event');

-- A free event the fixture admin holds a ready ticket for: its qr_url is set,
-- so the landing's inline ticket (.plans/038 P2-a) has a QR to expand.
-- 2030-01-22 12:00 → 15:00 UTC+7.
INSERT OR REPLACE INTO events (
  id, name, slug, status, event_format, event_start_ms, event_end_ms,
  deposit_enabled, deposit_amount_usdc, deposit_amount_thb, location, tagline,
  organizer_emails, sheet_id, sheet_name, staff_sheet_name, capacity,
  total_attendees, created_at, updated_at, link, visibility,
  in_person_capacity, online_capacity, description
) VALUES (
  'e2e-free', 'E2E Free Workshop', 'e2e-free-workshop', 'active', 'in_person',
  1895288400000, 1895299200000,
  0, 0, 0, 'Silom', 'Free fixture event',
  'organizer@example.com', 'e2e-no-sheet', 'Attendees', 'staff', 30,
  1, '2029-12-01T00:00:00Z', '2029-12-01T00:00:00Z', 'https://example.com', 'public',
  30, -1, 'A free event whose ticket is ready.'
);

INSERT OR REPLACE INTO attendees (
  id, event_id, email, name, approval_status, participation_type, claim_token,
  qr_url, deposit_status, deposit_amount_usdc, deposit_amount_thb, created_at,
  updated_at, consent_marketing, registration_phase
) VALUES
  ('e2e-att-04', 'e2e-free', 'e2e-admin@example.com', 'Somchai Example', 'approved', 'in-person',
   '0190e2e0-0000-7000-8000-000000000004', 'http://localhost:8788/staff/?scan=e2e-att-04',
   'none', 0, 0, '2029-12-03 10:00:00', '2029-12-03 10:00:00', 0, 'pre_event');
