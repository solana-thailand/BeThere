-- 0041 — Venue map link.
--
-- `location` is free text ("True Digital Park, Bangkok"). Organisers also want
-- a tappable map pin, so store the share link (e.g. Google Maps) next to it.
-- Empty = no link; the public pages keep showing the plain text.
ALTER TABLE events ADD COLUMN location_map_url TEXT DEFAULT '';
