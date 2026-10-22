SELECT q.declared_id AS requirement FROM documents q
WHERE q.type = 'requirement' AND q.metadata['priority'] = 'critical'
AND NOT EXISTS (
  SELECT 1 FROM relations r JOIN documents t ON t.internal_id = r.source_document_id
  WHERE r.target_document_id = q.internal_id AND r.kind = 'frontMatter.requirements'
    AND r.resolution_status = 'resolved' AND t.type = 'test-case' AND t.metadata['status'] = 'active'
)
ORDER BY q.declared_id
