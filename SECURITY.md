# Security policy

Report security issues privately through GitHub Security Advisories for this repository.

Alchetron defaults to local-only operation, denies private-network source retrieval, requires HTTPS for remote acquisition, limits redirects and response sizes, and protects mutations with role tokens plus same-origin checks. Model credentials are read from environment variables and are never written to audit records.

Do not expose `algo ui --allow-remote` directly to the internet. Place it behind an authenticated TLS reverse proxy and provision separate researcher/reviewer tokens.
