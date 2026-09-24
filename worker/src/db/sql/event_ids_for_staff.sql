SELECT id
FROM events
WHERE INSTR(',' || REPLACE(LOWER(staff_emails),' ','') || ',', ',' || ?1 || ',') > 0
ORDER BY created_at DESC, id DESC
LIMIT ?2
