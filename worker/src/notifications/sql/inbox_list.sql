SELECT * FROM notification_inbox_visible
WHERE lower(email)=lower(?1) AND id<?2
ORDER BY id DESC LIMIT 50
