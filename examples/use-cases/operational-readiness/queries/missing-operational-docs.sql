SELECT s.declared_id AS service FROM documents s
WHERE s.type = 'service' AND s.metadata->>'lifecycle' = 'production'
AND (NOT EXISTS (SELECT 1 FROM relations r WHERE r.source_document_id = s.internal_id AND r.kind = 'frontMatter.runbook' AND r.resolution_status = 'resolved')
  OR NOT EXISTS (SELECT 1 FROM relations r WHERE r.source_document_id = s.internal_id AND r.kind = 'frontMatter.deployment' AND r.resolution_status = 'resolved'))
ORDER BY s.declared_id
