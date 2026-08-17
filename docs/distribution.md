# Distribution

Tagged releases produce Windows, Linux, and macOS runtime packages containing the binary plus agents, schemas, registry, UI, fixtures, evaluation corpus, and SQL assets. GitHub generates build provenance attestations and `SHA256SUMS`.

- Windows: `./install.ps1`
- Linux/macOS: `./install.sh`
- Container: `docker compose up --build`

After installation, run `algo doctor`, `algo eval run`, and `algo ui`.
