-- 0063_door_clicks.sql — .plans/045 R4.10: which doors people take, counted
-- without cookies. One row per UTC day × page × door, and only a count: no
-- id, no IP, no user agent, nothing that names or follows a person
-- (worker/tests/door_clicks.rs pins the columns). Written only by
-- POST /api/public/click (worker/src/door_clicks.rs).
CREATE TABLE IF NOT EXISTS door_clicks (
    day TEXT NOT NULL,
    page TEXT NOT NULL,
    door TEXT NOT NULL,
    n INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, page, door)
);
