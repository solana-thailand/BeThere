-- Issue 051 Phase 2: Campaign reward tracking columns
-- Stores the minted NFT details when a developer claims their campaign reward.

ALTER TABLE developer_campaign_progress
  ADD COLUMN reward_asset_id TEXT;

ALTER TABLE developer_campaign_progress
  ADD COLUMN reward_signature TEXT;
