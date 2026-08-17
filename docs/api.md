# HTTP API

Alchetron exposes a versioned API from `algo ui` under `/api/v1`.

Public reads:

- `GET /api/v1/capabilities`
- `GET /api/v1/catalog`
- `GET /api/v1/graph`
- `GET /api/v1/emergent/logs`
- `GET /api/v1/emergent/hypotheses`

Authenticated calls use `Authorization: Bearer <token>` or the local UI header. The operator token is printed once by `algo ui`. Optional researcher and reviewer tokens come from `ALCHETRON_RESEARCHER_TOKEN` and `ALCHETRON_REVIEWER_TOKEN`.

Researcher actions create private hypotheses, challenges, logs, and disputes. Reviewer actions record human decisions. Operator actions include integrity inspection and rollback. No API exposed to a model can publish canonical truth.

The zero-dependency JavaScript client lives in `sdk/typescript`.
