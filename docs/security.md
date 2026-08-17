# Security model

- Live retrieval is disabled by default.
- HTTPS is required by default.
- Every redirect is processed and validated by Alchetron.
- Every network destination is DNS-resolved before use; private, loopback, link-local, multicast, unspecified, and reserved addresses are rejected for IPv4 and IPv6.
- Local locators are contained beneath `fixtures/` or `sources/`; traversal and symlink escapes are rejected.
- Extracted code is never executed. Python verification uses isolated AST parsing and reports `SYNTAX_ONLY`.
- The operator server binds to loopback unless `--allow-remote` is explicitly supplied.
- Browser mutations require a high-entropy per-process token, a custom header, matching origin, exact ID confirmation, reviewer, reason, and candidate hash.
- Canonical revisions and publication receipts are immutable. Rollback changes the canonical pointer and emits another receipt.

`--allow-remote` is intended only behind a separately authenticated TLS reverse proxy.
