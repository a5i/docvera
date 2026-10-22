SELECT e.declared_id AS epic, s.declared_id AS specification, s.metadata['status'] AS specification_status
FROM documents e JOIN relations r ON r.source_document_id = e.internal_id
JOIN documents s ON s.internal_id = r.target_document_id
WHERE e.type = 'jira-epic' AND e.metadata['status'] = 'done'
  AND r.kind = 'frontMatter.specification' AND r.resolution_status = 'resolved'
  AND s.type = 'specification' AND s.metadata['status'] != 'approved'
ORDER BY e.declared_id, s.declared_id
