# Canonical recovery

Each acceptance creates `data/archive/revisions/REV-<id>/` before moving `CURRENT.json`. A revision contains the full canonical catalog, accepted details, hash, previous revision, and commit time.

List revisions:

```powershell
cargo run --bin algo -- archive history
```

Restore one:

```powershell
cargo run --bin algo -- archive rollback REV-... --reason "restore last verified canon"
```

Rollback never deletes later revisions. It writes a new receipt and audit event. `CURRENT.json` is authoritative; root-level archive JSON files are compatibility exports.
