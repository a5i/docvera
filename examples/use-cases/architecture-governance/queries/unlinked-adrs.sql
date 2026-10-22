SELECT a.declared_id AS adr FROM documents a
WHERE a.type = 'adr' AND a.metadata['status'] = 'accepted'
AND NOT EXISTS (SELECT 1 FROM relations r WHERE r.source_document_id = a.internal_id
  AND r.kind = 'frontMatter.components' AND r.resolution_status = 'resolved')
ORDER BY a.declared_id
