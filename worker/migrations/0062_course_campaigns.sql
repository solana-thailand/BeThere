-- 0062_course_campaigns.sql — .plans/045 R4.4, owner 2026-10-11: courses are
-- campaigns, not a list in the code. Road to Mainnet and Solana in Latent
-- Space continue by date: an organizer adds the next event to the campaign
-- and it becomes the next episode (ordered by event_start_ms), with its
-- recording from the event's own video_url (set in the event form, through
-- PUT /api/events/{id}, never by a D1 write: the event read path is KV-first).
--
-- Seeds the two courses and links the events that exist here, found by slug
-- (event ids are not always their slugs). Re-running changes nothing.
INSERT OR IGNORE INTO campaigns (id, title, description, status)
VALUES
    ('road-to-mainnet', 'Road to Mainnet', 'Solana and AI from people who build it, once a month. Bangkok meetups.', 'active'),
    ('solana-in-latent-space', 'Solana in Latent Space', 'Rust, Solana and AI, one step a week. Online workshops.', 'active');

INSERT OR IGNORE INTO campaign_events (campaign_id, event_id, sequence_order)
SELECT 'road-to-mainnet', id, 0 FROM events WHERE slug IN (
    'solana-x-ai-builders-the-road-to-mainnet-1-bangkok',
    'solana-x-ai-builders-the-road-to-mainnet-2-bangkok',
    'solana-x-ai-builders-the-road-to-mainnet-3-bangkok',
    'solana-x-ai-builders-the-road-to-mainnet-4-bangkok',
    'solana-x-ai-builders-the-road-to-mainnet-5-bangkok',
    'solana-x-ai-builders-the-road-to-mainnet-6-bangkok'
);

INSERT OR IGNORE INTO campaign_events (campaign_id, event_id, sequence_order)
SELECT 'solana-in-latent-space', id, 0 FROM events WHERE slug IN (
    'solana-in-latent-space-part-1',
    'solana-in-latent-space-part-2',
    'solana-in-latent-space-part-3',
    'solana-in-latent-space-part-4',
    'solana-in-latent-space-part-5',
    'solana-in-latent-space-part-6'
);
