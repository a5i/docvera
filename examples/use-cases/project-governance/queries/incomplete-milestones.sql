SELECT m.declared_id AS milestone, e.declared_id AS deliverable
FROM documents m JOIN relations r ON r.source_document_id = m.internal_id
JOIN documents e ON e.internal_id = r.target_document_id
WHERE m.type = 'milestone' AND r.kind = 'frontMatter.deliverables' AND r.resolution_status = 'resolved'
  AND e.type = 'jira-epic' AND e.metadata['status'] != 'done'
ORDER BY m.declared_id, e.declared_id
