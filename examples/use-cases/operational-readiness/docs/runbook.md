---
id: RUN-001
owner: identity-team
reviewedAt: '2026-10-01'
---
# Diagnosis
For database connection timeout, inspect the connection pool and database health.
# Mitigation
Reduce concurrency and restore database connectivity.
# Escalation
Page identity-primary if recovery fails after five minutes.
