SELECT s.declared_id AS source, t.declared_id AS target
FROM relations r JOIN documents s ON s.internal_id = r.source_document_id
JOIN documents t ON t.internal_id = r.target_document_id
WHERE r.kind = 'frontMatter.dependsOn' AND r.resolution_status = 'resolved'
  AND s.type = 'component' AND t.type = 'component'
  AND s.metadata['layer'] = 'domain' AND t.metadata['layer'] = 'infrastructure'
ORDER BY source, target
