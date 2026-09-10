SELECT
  COUNT(*) FILTER (WHERE approval_status='approved') AS registered,
  COUNT(*) FILTER (WHERE checked_in_at IS NOT NULL) AS checked_in,
  COUNT(*) FILTER (WHERE claim_asset_id IS NOT NULL AND claim_asset_id<>'') AS claims_minted,
  (SELECT COUNT(*) FROM deposit_statuses
   WHERE event_id=?1 AND verified=1 AND method='usdc') AS deposits_verified,
  (SELECT COALESCE(SUM(amount),0) FROM deposit_statuses
   WHERE event_id=?1 AND verified=1 AND method='usdc') AS usdc_locked_total
FROM attendees WHERE event_id=?1
