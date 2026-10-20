SELECT d.declared_id AS requirement
FROM documents d
WHERE d.type = 'requirement' AND NOT EXISTS (
  SELECT 1 FROM relations r JOIN documents f ON f.internal_id = r.source_document_id
  WHERE r.target_document_id = d.internal_id AND r.resolution_status = 'resolved'
    AND r.kind = 'frontMatter.requirements' AND f.type = 'feature'
)
ORDER BY d.declared_id
