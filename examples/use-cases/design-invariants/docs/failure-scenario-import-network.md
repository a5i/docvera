---
id: FAIL-1
owner: identity-team
pattern: PTN-1
---
# Scenario
The network drops mid-import after a partial upload.
# Impact
Partial content must never be committed; the user gets a retry affordance.
