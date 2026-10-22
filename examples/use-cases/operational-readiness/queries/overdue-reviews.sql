SELECT declared_id AS runbook, metadata['reviewedAt'] AS reviewed_at
FROM documents WHERE type = 'runbook' AND metadata['reviewedAt'] < '2026-09-20'
ORDER BY declared_id
-- Evaluation date: 2026-10-20; review window: 30 days; cutoff: 2026-09-20.
-- ISO dates are compared lexically; no wall-clock dependency.
