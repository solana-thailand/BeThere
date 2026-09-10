UPDATE notification_outbox
SET status='uncertain',error_code='INTERRUPTED_SEND'
WHERE status='sending' AND attempted_at<unixepoch()-900
