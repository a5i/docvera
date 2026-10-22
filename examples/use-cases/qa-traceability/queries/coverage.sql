SELECT q.declared_id AS requirement,
  (SELECT count(DISTINCT t.internal_id) FROM relations r JOIN documents t ON t.internal_id = r.source_document_id
   WHERE r.target_document_id = q.internal_id AND r.kind = 'frontMatter.requirements'
     AND r.resolution_status = 'resolved' AND t.type = 'test-case'
     AND t.metadata['status'] = 'active') AS tests,
  (SELECT e.metadata['result'] FROM documents e
   JOIN relations et ON et.source_document_id = e.internal_id AND et.kind = 'frontMatter.test' AND et.resolution_status = 'resolved'
   JOIN documents t ON t.internal_id = et.target_document_id AND t.type = 'test-case' AND t.metadata['status'] = 'active'
   JOIN relations tr ON tr.source_document_id = t.internal_id AND tr.kind = 'frontMatter.requirements' AND tr.resolution_status = 'resolved'
   WHERE tr.target_document_id = q.internal_id AND e.type = 'test-execution'
     AND EXISTS (SELECT 1 FROM relations er JOIN documents rel ON rel.internal_id = er.target_document_id
                 WHERE er.source_document_id = e.internal_id AND er.kind = 'frontMatter.release'
                   AND er.resolution_status = 'resolved' AND rel.declared_id = 'REL-001')
   ORDER BY e.metadata['executedAt'] DESC, e.declared_id DESC LIMIT 1) AS latest_result
FROM documents q
WHERE q.type = 'requirement' AND EXISTS (
  SELECT 1 FROM relations rr JOIN documents rel ON rel.internal_id = rr.source_document_id
  WHERE rr.target_document_id = q.internal_id AND rr.kind = 'frontMatter.requirements'
    AND rr.resolution_status = 'resolved' AND rel.type = 'release' AND rel.declared_id = 'REL-001'
)
ORDER BY q.declared_id
