SELECT p.declared_id AS pattern
FROM documents p
WHERE p.type = 'interaction-pattern' AND p.metadata['lifecycle'] = 'stable'
  AND NOT EXISTS (
    SELECT 1 FROM relations r
    JOIN documents f ON f.internal_id = r.source_document_id
    WHERE r.kind = 'frontMatter.entity' AND r.resolution_status = 'resolved'
      AND r.target_document_id = p.internal_id
      AND f.type = 'finding' AND f.metadata['status'] = 'open')
  AND NOT EXISTS (
    SELECT 1 FROM relations r
    WHERE r.kind = 'frontMatter.implementedBy' AND r.resolution_status = 'resolved'
      AND r.source_document_id = p.internal_id)
ORDER BY pattern
