SELECT declared_id AS feature
FROM documents
WHERE type = 'feature' AND metadata['status'] IN ('planned', 'delivered')
  AND metadata['epic'] IS NULL
ORDER BY declared_id
