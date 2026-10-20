---
id: ADR-001
owner: architecture
status: accepted
components: [COMP-001, COMP-002]
---
# Context
Recovery rules must be independent of the database.
# Decision
Use a domain-owned persistence interface.
# Consequences
Storage adapters depend on the domain, not the reverse.
